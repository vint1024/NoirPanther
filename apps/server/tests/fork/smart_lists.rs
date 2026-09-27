use models::shared::enums::ReadingStatus;
use serde_json::json;
use tests::fake_data;

use crate::common::{series::setup_single_series_with_n_books, TestApp};

/// NoirPanther: a smart list filtered by reading status joins the reading sessions and groups the
/// books query by book; `meta` then selected `series.library_id` from that grouped query, which
/// PostgreSQL refuses ("must appear in the GROUP BY clause") — and the app loads every list's meta
/// in one request, so one such list broke the whole Smart lists screen. SQLite let it through, so
/// run this with `TEST_DATABASE_URL` against PostgreSQL too.
#[tokio::test]
async fn a_reading_status_list_counts_its_books_series_and_libraries() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("meta_lib".to_string()),
		name: Some("Meta library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let (_series, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("meta_series".to_string()),
			name: Some("Meta series".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		3,
	)
	.await;
	let owner_id = app.execute_gql(r#"query { me { id } }"#, None).await["data"]["me"]
		["id"]
		.as_str()
		.expect("viewer id")
		.to_string();

	// one being read, one finished, one untouched
	fake_data::ReadingSession {
		media_id: books[0].id.clone(),
		user_id: owner_id.clone(),
		end_percentage: 0.4,
		status: ReadingStatus::Reading,
		created_at: None,
	}
	.insert(app.conn())
	.await;
	fake_data::ReadingSession::completed(books[1].id.clone(), owner_id.clone())
		.insert(app.conn())
		.await;

	let created = app
		.execute_gql(
			r#"mutation Create($input: SaveSmartListInput!) { createSmartList(input: $input) { id } }"#,
			Some(json!({ "input": {
				"name": "Reading now",
				"joiner": "AND",
				"defaultGrouping": "BY_BOOKS",
				"visibility": "PRIVATE",
				"filters": [{
					"joiner": "AND",
					"groups": [{ "media": { "readingStatus": { "isAnyOf": ["READING"] } } }]
				}]
			}})),
		)
		.await;
	assert!(created.get("errors").is_none(), "{created:#}");
	let id = created["data"]["createSmartList"]["id"]
		.as_str()
		.expect("list id")
		.to_string();

	let lists = app
		.execute_gql(
			r#"query { smartLists { id meta { matchedBooks matchedSeries matchedLibraries } } }"#,
			None,
		)
		.await;
	assert!(
		lists.get("errors").is_none(),
		"a reading-status list broke the meta of every list: {lists:#}"
	);
	let meta = lists["data"]["smartLists"]
		.as_array()
		.expect("lists")
		.iter()
		.find(|list| list["id"] == id)
		.expect("the new list")["meta"]
		.clone();
	assert_eq!(
		meta,
		json!({ "matchedBooks": 1, "matchedSeries": 1, "matchedLibraries": 1 })
	);
}
