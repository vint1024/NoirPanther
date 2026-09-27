use chrono::Utc;
use models::{
	entity::{book_club, book_club_member},
	shared::book_club::BookClubMemberRole,
};
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};

use crate::common::{
	account::{CreateTestUser, TestUser},
	TestApp,
};

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

const CONSENT_CLUB: &str = "consent_club";

struct ConsentClub {
	admin: TestUser,
	member: TestUser,
	guest: TestUser,
}

/// A club run by `admin`, with `member` in it and `guest` on the server but not in the club.
async fn setup_consent_club(app: &TestApp) -> ConsentClub {
	book_club::Entity::insert(book_club::ActiveModel {
		id: ActiveValue::Set(CONSENT_CLUB.to_string()),
		name: ActiveValue::Set("Ask first".to_string()),
		slug: ActiveValue::Set("ask-first".to_string()),
		is_private: ActiveValue::Set(false),
		created_at: ActiveValue::Set(Utc::now().into()),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("club insert");

	let mut users = Vec::new();
	for (name, role) in [
		("club-admin", Some(BookClubMemberRole::Admin)),
		("club-member", Some(BookClubMemberRole::Member)),
		("club-guest", None),
	] {
		let user = CreateTestUser {
			username: name.to_string(),
			password: "password".to_string(),
			..Default::default()
		}
		.insert(app)
		.await;
		if let Some(role) = role {
			book_club_member::Entity::insert(book_club_member::ActiveModel {
				id: ActiveValue::Set(format!("{name}-membership")),
				hide_progress: ActiveValue::Set(false),
				role: ActiveValue::Set(role),
				joined_at: ActiveValue::Set(Utc::now().into()),
				user_id: ActiveValue::Set(user.id.clone()),
				book_club_id: ActiveValue::Set(CONSENT_CLUB.to_string()),
				..Default::default()
			})
			.exec(app.conn())
			.await
			.expect("member insert");
		}
		users.push(user);
	}
	let guest = users.pop().unwrap();
	let member = users.pop().unwrap();
	let admin = users.pop().unwrap();
	ConsentClub {
		admin,
		member,
		guest,
	}
}

async fn memberships_of(app: &TestApp, user: &TestUser) -> usize {
	book_club_member::Entity::find()
		.filter(book_club_member::Column::BookClubId.eq(CONSENT_CLUB))
		.filter(book_club_member::Column::UserId.eq(user.id.as_str()))
		.all(app.conn())
		.await
		.expect("members")
		.len()
}

async fn invite(app: &TestApp, by: &TestUser, who: &TestUser) -> Value {
	app.execute_gql_with_token(
		r#"mutation Invite($id: ID!, $input: BookClubInvitationInput!) {
            createBookClubInvitation(id: $id, input: $input) { id }
        }"#,
		Some(json!({ "id": CONSENT_CLUB, "input": { "userId": who.id } })),
		&by.token,
	)
	.await
}

async fn my_invitations(app: &TestApp, user: &TestUser) -> Vec<String> {
	let response = app
		.execute_gql_with_token(
			"query { myBookClubInvitations { id bookClubId } }",
			None,
			&user.token,
		)
		.await;
	assert!(response.get("errors").is_none(), "{response:#}");
	response["data"]["myBookClubInvitations"]
		.as_array()
		.expect("invitations")
		.iter()
		.map(|i| i["id"].as_str().expect("id").to_string())
		.collect()
}

async fn respond(app: &TestApp, user: &TestUser, id: &str, accept: bool) -> Value {
	let input = if accept {
		json!({ "accept": true, "member": { "userId": user.id } })
	} else {
		json!({ "accept": false })
	};
	app.execute_gql_with_token(
		r#"mutation Respond($id: ID!, $input: BookClubInvitationResponseInput!) {
            respondToBookClubInvitation(id: $id, input: $input) { id }
        }"#,
		Some(json!({ "id": id, "input": input })),
		&user.token,
	)
	.await
}

/// NoirPanther (user, 2026-09-27): nobody is put in a book club without saying yes. Upstream's
/// `createBookClubMember` adds a person outright — it only never worked, `joined_at` was left
/// empty. If upstream fixes that, this test is what keeps direct adds from coming back.
#[tokio::test]
async fn nobody_is_added_to_a_club_without_saying_yes() {
	let app = TestApp::new_with_default_user().await;
	let club = setup_consent_club(&app).await;
	let add = r#"mutation Add($id: ID!, $input: CreateBookClubMemberInput!) {
        createBookClubMember(bookClubId: $id, input: $input) { id }
    }"#;
	let vars = json!({ "id": CONSENT_CLUB, "input": { "userId": club.guest.id, "role": "MEMBER" } });

	let by_admin = app
		.execute_gql_with_token(add, Some(vars.clone()), &club.admin.token)
		.await;
	let by_owner = app.execute_gql(add, Some(vars)).await;

	for response in [by_admin, by_owner] {
		let message = response["errors"][0]["message"]
			.as_str()
			.unwrap_or_default();
		assert!(
			message.contains("invitation"),
			"a direct add was not refused with a pointer to invitations: {response:#}"
		);
	}
	assert_eq!(memberships_of(&app, &club.guest).await, 0);
}

/// The invited person decides: declining leaves them out, accepting puts them in — with a joined
/// date, so the row is valid on PostgreSQL too.
#[tokio::test]
async fn the_invited_person_accepts_or_declines() {
	let app = TestApp::new_with_default_user().await;
	let club = setup_consent_club(&app).await;

	let sent = invite(&app, &club.admin, &club.guest).await;
	assert!(sent.get("errors").is_none(), "{sent:#}");
	let pending = my_invitations(&app, &club.guest).await;
	assert_eq!(pending.len(), 1);
	let declined = respond(&app, &club.guest, &pending[0], false).await;
	assert!(declined.get("errors").is_none(), "{declined:#}");
	assert_eq!(memberships_of(&app, &club.guest).await, 0);
	assert!(my_invitations(&app, &club.guest).await.is_empty());

	let again = invite(&app, &club.admin, &club.guest).await;
	assert!(
		again.get("errors").is_none(),
		"a declined invitation blocked a new one: {again:#}"
	);
	let pending = my_invitations(&app, &club.guest).await;
	let accepted = respond(&app, &club.guest, &pending[0], true).await;
	assert!(accepted.get("errors").is_none(), "{accepted:#}");
	assert_eq!(memberships_of(&app, &club.guest).await, 1);
	assert!(my_invitations(&app, &club.guest).await.is_empty());
}

/// Upstream stored every invitation: the same club showed up twice for the invitee, and people
/// already in the club could be invited into it again.
#[tokio::test]
async fn the_same_person_is_not_invited_twice() {
	let app = TestApp::new_with_default_user().await;
	let club = setup_consent_club(&app).await;

	let first = invite(&app, &club.admin, &club.guest).await;
	assert!(first.get("errors").is_none(), "{first:#}");
	let second = invite(&app, &club.admin, &club.guest).await;
	assert!(
		second["errors"][0]["message"]
			.as_str()
			.unwrap_or_default()
			.contains("already has an invitation"),
		"{second:#}"
	);
	let member = invite(&app, &club.admin, &club.member).await;
	assert!(
		member["errors"][0]["message"]
			.as_str()
			.unwrap_or_default()
			.contains("already in the club"),
		"{member:#}"
	);
	assert_eq!(my_invitations(&app, &club.guest).await.len(), 1);
	assert!(my_invitations(&app, &club.member).await.is_empty());
}

/// Who has been invited is for the people who invite: the club's admins and the server owner.
/// Anyone else gets an empty list rather than an error.
#[tokio::test]
async fn only_the_clubs_admins_see_who_is_invited() {
	let app = TestApp::new_with_default_user().await;
	let club = setup_consent_club(&app).await;
	let sent = invite(&app, &club.admin, &club.guest).await;
	assert!(sent.get("errors").is_none(), "{sent:#}");

	let query =
		r#"query Invited($id: ID!) { bookClubById(id: $id) { invitations { userId } } }"#;
	let invited = |response: &Value| -> Vec<String> {
		assert!(response.get("errors").is_none(), "{response:#}");
		response["data"]["bookClubById"]["invitations"]
			.as_array()
			.expect("invitations")
			.iter()
			.map(|i| i["userId"].as_str().expect("userId").to_string())
			.collect()
	};
	let vars = Some(json!({ "id": CONSENT_CLUB }));

	let as_admin = app
		.execute_gql_with_token(query, vars.clone(), &club.admin.token)
		.await;
	let as_owner = app.execute_gql(query, vars.clone()).await;
	let as_member = app
		.execute_gql_with_token(query, vars, &club.member.token)
		.await;

	assert_eq!(invited(&as_admin), vec![club.guest.id.clone()]);
	assert_eq!(invited(&as_owner), vec![club.guest.id.clone()]);
	assert!(invited(&as_member).is_empty());
}

/// Only a club's admins (and the creator) invite. Upstream checked the role with `role >= 2`
/// against a column that holds role *names*: PostgreSQL refused the query outright, and SQLite —
/// where any text sorts above any number — let every plain member act as an admin.
#[tokio::test]
async fn a_plain_member_cannot_invite() {
	let app = TestApp::new_with_default_user().await;
	let club = setup_consent_club(&app).await;

	let by_member = invite(&app, &club.member, &club.guest).await;
	assert!(
		by_member.get("errors").is_some(),
		"a plain member sent an invitation: {by_member:#}"
	);
	assert!(my_invitations(&app, &club.guest).await.is_empty());

	let by_admin = invite(&app, &club.admin, &club.guest).await;
	assert!(
		by_admin.get("errors").is_none(),
		"the club's admin could not invite: {by_admin:#}"
	);
}
