use chrono::Utc;
use models::{
	entity::{book_club, book_club_member},
	shared::book_club::BookClubMemberRole,
};
use sea_orm::{ActiveValue, EntityTrait};
use serde_json::json;

use crate::common::{account::CreateTestUser, TestApp};

const CLUB_ID: &str = "big_club";

async fn setup_club_with_members(app: &TestApp, members: usize) -> Vec<String> {
	book_club::Entity::insert(book_club::ActiveModel {
		id: ActiveValue::Set(CLUB_ID.to_string()),
		name: ActiveValue::Set("A club with a crowd".to_string()),
		slug: ActiveValue::Set("a-club-with-a-crowd".to_string()),
		is_private: ActiveValue::Set(false),
		created_at: ActiveValue::Set(Utc::now().into()),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("club insert");

	let mut ids = Vec::new();
	for i in 0..members {
		let user = CreateTestUser {
			username: format!("clubber-{i}"),
			password: "password".to_string(),
			..Default::default()
		}
		.insert(app)
		.await;

		// ids are ordered here on purpose: the cursor pages by id ascending
		let member_id = format!("member_{i:02}");
		book_club_member::Entity::insert(book_club_member::ActiveModel {
			id: ActiveValue::Set(member_id.clone()),
			display_name: ActiveValue::Set(Some(format!("Clubber {i}"))),
			hide_progress: ActiveValue::Set(false),
			role: ActiveValue::Set(BookClubMemberRole::Member),
			joined_at: ActiveValue::Set(Utc::now().into()),
			user_id: ActiveValue::Set(user.id.clone()),
			book_club_id: ActiveValue::Set(CLUB_ID.to_string()),
			..Default::default()
		})
		.exec(app.conn())
		.await
		.expect("member insert");
		ids.push(member_id);
	}
	ids
}

/// A10: a club's `members` array loads everyone at once, which is fine for a reading group and
/// not fine for the clubs this fork is built for. `bookClubMembers` pages by keyset instead — and
/// the property that matters is not "it returns rows" but "walking the pages yields every member
/// exactly once", which an off-by-one in the cursor quietly breaks.
#[tokio::test]
async fn paging_through_the_members_yields_each_of_them_exactly_once() {
	let app = TestApp::new_with_default_user().await;
	let expected = setup_club_with_members(&app, 7).await;

	let mut seen: Vec<String> = Vec::new();
	let mut cursor: Option<String> = None;
	// generous bound: with limit 2 over 7 members this needs 4 pages, never 10
	for page in 0..10 {
		let response = app
			.execute_gql(
				r#"query Page($id: ID!, $after: String) {
                    bookClubMembers(bookClubId: $id, pagination: { limit: 2, after: $after }) {
                        nodes { id }
                        cursorInfo { nextCursor limit }
                    }
                }"#,
				Some(json!({ "id": CLUB_ID, "after": cursor })),
			)
			.await;
		assert!(
			response.get("errors").is_none(),
			"page {page} failed: {response:#}"
		);
		let page_data = &response["data"]["bookClubMembers"];
		let ids = page_data["nodes"]
			.as_array()
			.expect("nodes")
			.iter()
			.filter_map(|node| node["id"].as_str().map(str::to_string))
			.collect::<Vec<_>>();
		assert!(ids.len() <= 2, "the page ignored the limit: {page_data:#}");
		seen.extend(ids);

		match page_data["cursorInfo"]["nextCursor"].as_str() {
			Some(next) => cursor = Some(next.to_string()),
			None => break,
		}
	}

	assert_eq!(
		seen, expected,
		"paging did not walk the members once each, in order"
	);
}

/// A10: the club roster is still access-controlled. A private club's members must not be listed
/// by someone outside it, whichever door they use.
#[tokio::test]
async fn an_outsider_cannot_page_through_a_private_clubs_members() {
	let app = TestApp::new_with_default_user().await;
	setup_club_with_members(&app, 3).await;

	book_club::Entity::update(book_club::ActiveModel {
		id: ActiveValue::Set(CLUB_ID.to_string()),
		is_private: ActiveValue::Set(true),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("make the club private");

	let outsider = CreateTestUser {
		username: "outsider".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			r#"query Page($id: ID!) {
                bookClubMembers(bookClubId: $id, pagination: { limit: 10 }) { nodes { id } }
            }"#,
			Some(json!({ "id": CLUB_ID })),
			&outsider.token,
		)
		.await;

	assert!(
		response.get("errors").is_some(),
		"an outsider paged through a private club's roster: {response:#}"
	);
}
