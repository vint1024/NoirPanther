use models::entity::media_metadata;
use sea_orm::{ActiveValue, EntityTrait};
use serde_json::json;
use tests::fake_data;

use crate::common::{series::setup_single_series_with_n_books, TestApp};

/// A11/A12: search matches regardless of letter case in any language (SQLite gets a custom
/// `ulower`, PostgreSQL a SQL wrapper), and it matches the people who wrote the book, not only
/// its title. Upstream's `lower()` only folds ASCII, so a Russian query in lowercase found
/// nothing at all.
async fn setup() -> (TestApp, String) {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("search_lib".to_string()),
		name: Some("Search library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (_, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("search_series".to_string()),
			name: Some("Search series".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;
	let book = books.into_iter().next().expect("one book");

	media_metadata::Entity::insert(media_metadata::ActiveModel {
		media_id: ActiveValue::Set(Some(book.id.clone())),
		title: ActiveValue::Set(Some("Война и Мир".to_string())),
		writers: ActiveValue::Set(Some("Лев Толстой".to_string())),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("metadata insert");

	(app, book.id)
}

async fn search_titles(app: &TestApp, query: &str) -> Vec<String> {
	let response = app
		.execute_gql(
			r#"query Find($q: String!) {
				media(filter: { metadata: { title: { contains: $q } } },
					  pagination: { offset: { page: 1, pageSize: 20 } }) { nodes { id } }
			}"#,
			Some(json!({ "q": query })),
		)
		.await;
	response["data"]["media"]["nodes"]
		.as_array()
		.map(|nodes| {
			nodes
				.iter()
				.filter_map(|node| node["id"].as_str().map(str::to_string))
				.collect()
		})
		.unwrap_or_default()
}

#[tokio::test]
async fn search_ignores_letter_case_in_any_language() {
	let (app, book_id) = setup().await;

	for query in ["война", "ВОЙНА", "Война"] {
		let found = search_titles(&app, query).await;
		assert!(
			found.contains(&book_id),
			"`{query}` did not match the book — Unicode case folding is gone"
		);
	}
}

#[tokio::test]
async fn search_also_matches_the_author() {
	let (app, book_id) = setup().await;

	let response = app
		.execute_gql(
			r#"query Find($q: String!) {
				media(filter: { metadata: { writers: { contains: $q } } },
					  pagination: { offset: { page: 1, pageSize: 20 } }) { nodes { id } }
			}"#,
			Some(json!({ "q": "толстой" })),
		)
		.await;

	let ids = response["data"]["media"]["nodes"]
		.as_array()
		.expect("media list")
		.iter()
		.filter_map(|node| node["id"].as_str())
		.collect::<Vec<_>>();
	assert!(
		ids.contains(&book_id.as_str()),
		"searching by the author's name found nothing: {response:#}"
	);
}
