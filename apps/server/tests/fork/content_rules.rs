use models::{
	entity::{content_access_rule, media_metadata},
	shared::enums::{ContentRuleDimension, ContentRuleMode},
};
use sea_orm::{ActiveValue, EntityTrait};
use serde_json::json;
use tests::fake_data;

use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};

/// A3/A31: a rule hides matching books from a user — books, not just series. The v0.1.7 merge
/// kept the rule code but dropped the calls to it, and rule-hidden books were visible again for
/// eight days. `visibility_filters_are_applied` guards the SQL; this guards the whole path,
/// through the API, the way a user would hit it.
#[tokio::test]
async fn a_content_rule_hides_the_book_from_the_restricted_user() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("rules_lib".to_string()),
		name: Some("Rules library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (_, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("rules_series".to_string()),
			name: Some("Rules series".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		2,
	)
	.await;
	let hidden = books.first().expect("two books were created");
	let visible = books.get(1).expect("two books were created");

	// tag the first book with a genre we will then exclude
	media_metadata::Entity::insert(media_metadata::ActiveModel {
		media_id: ActiveValue::Set(Some(hidden.id.clone())),
		genres: ActiveValue::Set(Some("Horror".to_string())),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("metadata insert");

	let reader = CreateTestUser {
		username: "restricted-reader".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	content_access_rule::Entity::insert(content_access_rule::ActiveModel {
		user_id: ActiveValue::Set(reader.id.clone()),
		dimension: ActiveValue::Set(ContentRuleDimension::Genre),
		mode: ActiveValue::Set(ContentRuleMode::Exclude),
		values: ActiveValue::Set(serde_json::json!(["Horror"])),
		restrict_on_unset: ActiveValue::Set(false),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("rule insert");

	let listed = app
		.execute_gql_with_token(
			r#"query { media(pagination: { offset: { page: 1, pageSize: 50 } }) { nodes { id } } }"#,
			None,
			&reader.token,
		)
		.await;
	let ids = listed["data"]["media"]["nodes"]
		.as_array()
		.expect("media list")
		.iter()
		.filter_map(|node| node["id"].as_str())
		.collect::<Vec<_>>();

	assert!(
		!ids.contains(&hidden.id.as_str()),
		"the rule-hidden book is still listed: {listed:#}"
	);
	assert!(
		ids.contains(&visible.id.as_str()),
		"the rule hid a book it should not have: {listed:#}"
	);

	// …and it must not be reachable by id either
	let by_id = app
		.execute_gql_with_token(
			r#"query Book($id: ID!) { mediaById(id: $id) { id } }"#,
			Some(json!({ "id": hidden.id })),
			&reader.token,
		)
		.await;
	assert!(
		by_id["data"]["mediaById"].is_null(),
		"the rule-hidden book is reachable by id: {by_id:#}"
	);
}
