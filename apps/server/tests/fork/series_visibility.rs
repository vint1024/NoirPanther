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

/// A4: hiding every book of a series must hide the series too, otherwise a restricted user is
/// shown a series that opens empty. The filter is a subquery bolted onto `find_for_user`, so an
/// upstream rewrite of the series query drops it silently — nothing fails to compile.
#[tokio::test]
async fn a_series_whose_books_are_all_hidden_disappears_from_the_listing() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("visibility_lib".to_string()),
		name: Some("Visibility library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	// a series where BOTH books are horror — the user should not see the series at all
	let (all_horror, horror_books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("all_horror".to_string()),
			name: Some("All horror".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		2,
	)
	.await;

	// a series where only one book is horror — it stays, with one book left
	let (partly_horror, mixed_books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("partly_horror".to_string()),
			name: Some("Partly horror".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		2,
	)
	.await;

	// a series with no books at all: emptiness is a different concern, it must stay visible
	let empty = fake_data::Series {
		id: Some("no_books".to_string()),
		name: Some("No books".to_string()),
		library_id: Some(library.id.clone()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	for book in horror_books
		.iter()
		.chain(mixed_books.iter().take(1))
		.collect::<Vec<_>>()
	{
		media_metadata::Entity::insert(media_metadata::ActiveModel {
			media_id: ActiveValue::Set(Some(book.id.clone())),
			genres: ActiveValue::Set(Some("Horror".to_string())),
			..Default::default()
		})
		.exec(app.conn())
		.await
		.expect("metadata insert");
	}

	let reader = CreateTestUser {
		username: "no-horror".to_string(),
		password: "password".to_string(),
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

	let listed = app
		.execute_gql_with_token(
			r#"query { series(pagination: { offset: { page: 1, pageSize: 50 } }) { nodes { id } } }"#,
			None,
			&reader.token,
		)
		.await;
	let ids = listed["data"]["series"]["nodes"]
		.as_array()
		.expect("series list")
		.iter()
		.filter_map(|node| node["id"].as_str())
		.collect::<Vec<_>>();

	assert!(
		!ids.contains(&all_horror.id.as_str()),
		"a series with nothing visible in it is still listed: {listed:#}"
	);
	assert!(
		ids.contains(&partly_horror.id.as_str()),
		"a series with one visible book was hidden: {listed:#}"
	);
	assert!(
		ids.contains(&empty.id.as_str()),
		"a series with no books at all was hidden: {listed:#}"
	);

	// the owner has no rules, so nothing is hidden from them
	let as_owner = app
		.execute_gql(
			r#"query { series(pagination: { offset: { page: 1, pageSize: 50 } }) { nodes { id } } }"#,
			None,
		)
		.await;
	let owner_ids = as_owner["data"]["series"]["nodes"]
		.as_array()
		.expect("series list")
		.iter()
		.filter_map(|node| node["id"].as_str())
		.collect::<Vec<_>>();
	assert!(
		owner_ids.contains(&all_horror.id.as_str()),
		"the rule leaked onto a user who has none: {as_owner:#}"
	);
}
