use models::{
	entity::{content_access_rule, media_metadata},
	shared::enums::{ContentRuleDimension, ContentRuleMode, UserPermission},
};
use sea_orm::{ActiveValue, EntityTrait};
use serde_json::json;
use tests::fake_data;

use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};

/// A30: `updateUser` took the permission list straight from the input, so any user could hand
/// themselves every permission by editing their own profile. A self-update must never touch
/// permissions, the age restriction or the session cap.
#[tokio::test]
async fn a_user_cannot_grant_themselves_permissions() {
	let app = TestApp::new_with_default_user().await;

	let user = CreateTestUser {
		username: "climber".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			r#"mutation Escalate($id: ID!, $input: UpdateUserInput!) {
                updateUser(id: $id, input: $input) { id permissions }
            }"#,
			Some(json!({
				"id": user.id,
				"input": {
					"username": "climber",
					"permissions": ["MANAGE_LIBRARY", "MANAGE_USERS", "MANAGE_SERVER"],
					"maxSessionsAllowed": 999,
				},
			})),
			&user.token,
		)
		.await;

	// whether the mutation refuses outright or quietly ignores the field, the account must come
	// out of it with nothing new
	let permissions = app
		.execute_gql(
			r#"query Check($id: ID!) { userById(id: $id) { permissions } }"#,
			Some(json!({ "id": user.id })),
		)
		.await;
	let granted = permissions["data"]["userById"]["permissions"]
		.as_array()
		.expect("permissions list")
		.iter()
		.filter_map(|value| value.as_str())
		.collect::<Vec<_>>();
	assert!(
		granted.is_empty(),
		"a user granted themselves {granted:?} by editing their own profile: {response:#}"
	);
}

/// A32: `libraryMissingEntities` ran raw SQL with no guard, so any account could list the file
/// paths of every library on the server. It is an administrative view and must stay one.
#[tokio::test]
async fn listing_a_librarys_missing_files_requires_managing_libraries() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("audit_lib".to_string()),
		name: Some("Audit library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let nosy = CreateTestUser {
		username: "nosy".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			r#"query Missing($id: ID!) {
                libraryMissingEntities(libraryId: $id, pagination: { offset: { page: 1, pageSize: 10 } }) {
                    nodes { path }
                }
            }"#,
			Some(json!({ "id": library.id })),
			&nosy.token,
		)
		.await;
	assert!(
		response.get("errors").is_some(),
		"a user without ManageLibrary listed a library's file paths: {response:#}"
	);

	// the owner still gets the view — the guard must not break the feature
	let as_owner = app
		.execute_gql(
			r#"query Missing($id: ID!) {
                libraryMissingEntities(libraryId: $id, pagination: { offset: { page: 1, pageSize: 10 } }) {
                    nodes { path }
                }
            }"#,
			Some(json!({ "id": library.id })),
		)
		.await;
	assert!(
		as_owner.get("errors").is_none(),
		"the guard also locked out an administrator: {as_owner:#}"
	);
}

/// A33: the metadata overview (the values offered in filter dropdowns) was built from an
/// unfiltered query, so a restricted user was shown the genres and authors of the very books the
/// content rules hide from them — the listing was filtered, the vocabulary was not.
#[tokio::test]
async fn the_metadata_overview_does_not_leak_hidden_values() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("overview_lib".to_string()),
		name: Some("Overview library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (_, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("overview_series".to_string()),
			name: Some("Overview series".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		2,
	)
	.await;

	media_metadata::Entity::insert(media_metadata::ActiveModel {
		media_id: ActiveValue::Set(Some(books[0].id.clone())),
		genres: ActiveValue::Set(Some("Horror".to_string())),
		writers: ActiveValue::Set(Some("Hidden Author".to_string())),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("metadata insert");
	media_metadata::Entity::insert(media_metadata::ActiveModel {
		media_id: ActiveValue::Set(Some(books[1].id.clone())),
		genres: ActiveValue::Set(Some("Poetry".to_string())),
		writers: ActiveValue::Set(Some("Visible Author".to_string())),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("metadata insert");

	let reader = CreateTestUser {
		username: "overview-reader".to_string(),
		password: "password".to_string(),
		permissions: vec![UserPermission::ReadNotifier],
		..Default::default()
	}
	.insert(&app)
	.await;

	content_access_rule::Entity::insert(content_access_rule::ActiveModel {
		user_id: ActiveValue::Set(reader.id.clone()),
		dimension: ActiveValue::Set(ContentRuleDimension::Genre),
		mode: ActiveValue::Set(ContentRuleMode::Exclude),
		values: ActiveValue::Set(json!(["Horror"])),
		restrict_on_unset: ActiveValue::Set(false),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("rule insert");

	let overview = app
		.execute_gql_with_token(
			r#"query { mediaMetadataOverview { genres writers } }"#,
			None,
			&reader.token,
		)
		.await;
	let genres = overview["data"]["mediaMetadataOverview"]["genres"]
		.as_array()
		.expect("genres")
		.iter()
		.filter_map(|value| value.as_str())
		.collect::<Vec<_>>();
	let writers = overview["data"]["mediaMetadataOverview"]["writers"]
		.as_array()
		.expect("writers")
		.iter()
		.filter_map(|value| value.as_str())
		.collect::<Vec<_>>();

	assert!(
		!genres.contains(&"Horror"),
		"the overview offered a genre the user may not see: {overview:#}"
	);
	assert!(
		!writers.contains(&"Hidden Author"),
		"the overview named an author from a hidden book: {overview:#}"
	);
	assert!(
		genres.contains(&"Poetry") && writers.contains(&"Visible Author"),
		"the filter also removed what the user is allowed to see: {overview:#}"
	);
}

/// A32 (second half): every sibling query checks club access; `previousBookClubDiscussions` did
/// not, so anyone who knew a club id could read a private club's past discussions.
#[tokio::test]
async fn previous_club_discussions_are_not_readable_from_outside_the_club() {
	use chrono::Utc;
	use models::entity::book_club;
	use sea_orm::EntityTrait as _;

	let app = TestApp::new_with_default_user().await;
	book_club::Entity::insert(book_club::ActiveModel {
		id: ActiveValue::Set("private_club".to_string()),
		name: ActiveValue::Set("Behind closed doors".to_string()),
		slug: ActiveValue::Set("behind-closed-doors".to_string()),
		is_private: ActiveValue::Set(true),
		created_at: ActiveValue::Set(Utc::now().into()),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("club insert");

	let outsider = CreateTestUser {
		username: "eavesdropper".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			r#"query Past($id: ID!) {
                previousBookClubDiscussions(bookClubId: $id) { id }
            }"#,
			Some(json!({ "id": "private_club" })),
			&outsider.token,
		)
		.await;
	assert!(
		response.get("errors").is_some(),
		"an outsider read a private club's past discussions: {response:#}"
	);
}
