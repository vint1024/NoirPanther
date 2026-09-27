use models::shared::enums::ReadingStatus;
use serde_json::json;
use tests::fake_data;

use crate::common::{series::setup_single_series_with_n_books, TestApp};

/// NoirPanther: `onDeck` counted with a second SQL body that had drifted from the list's — it
/// left out books being read but not series being read, and skipped the access filter — so
/// `totalItems` said 2 while the list held 1 (found on :10912, 2026-09-24). One list now feeds
/// both, filtered for access before it is paged.
#[tokio::test]
async fn on_deck_counts_exactly_the_books_it_lists() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("deck_lib".to_string()),
		name: Some("Deck library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let series = |id: &str| fake_data::Series {
		id: Some(id.to_string()),
		name: Some(id.to_string()),
		library_id: Some(library.id.clone()),
		..Default::default()
	};
	// "finished_one": book 1 read, so book 2 is next up
	let (_, finished_one) =
		setup_single_series_with_n_books(&app, series("finished_one"), 3).await;
	// "reading_now": book 1 read, book 2 being read — the series is in progress, not on deck
	let (_, reading_now) =
		setup_single_series_with_n_books(&app, series("reading_now"), 3).await;

	let owner_id = app.execute_gql(r#"query { me { id } }"#, None).await["data"]["me"]
		["id"]
		.as_str()
		.expect("viewer id")
		.to_string();
	for book in [&finished_one[0], &reading_now[0]] {
		fake_data::ReadingSession::completed(book.id.clone(), owner_id.clone())
			.insert(app.conn())
			.await;
	}
	fake_data::ReadingSession {
		media_id: reading_now[1].id.clone(),
		user_id: owner_id.clone(),
		end_percentage: 0.5,
		status: ReadingStatus::Reading,
		created_at: None,
	}
	.insert(app.conn())
	.await;

	let response = app
		.execute_gql(
			r#"query Deck($p: Pagination!) {
                onDeck(pagination: $p) {
                    nodes { id }
                    pageInfo { ... on OffsetPaginationInfo { totalItems totalPages } }
                }
            }"#,
			Some(json!({ "p": { "offset": { "page": 1, "pageSize": 20 } } })),
		)
		.await;
	assert!(response.get("errors").is_none(), "{response:#}");
	let deck = &response["data"]["onDeck"];
	let ids: Vec<&str> = deck["nodes"]
		.as_array()
		.expect("nodes")
		.iter()
		.filter_map(|n| n["id"].as_str())
		.collect();
	assert_eq!(ids, vec![finished_one[1].id.as_str()]);
	assert_eq!(
		deck["pageInfo"]["totalItems"], 1,
		"totalItems promised books the list doesn't show: {deck:#}"
	);
}
