use models::shared::enums::ReadingStatus;
use tests::fake_data;

use crate::common::{series::setup_single_series_with_n_books, TestApp};

/// A22: filtering series by `ABANDONED` was an `unimplemented!()` upstream — asking for it took
/// the whole server down, not just the request. We implemented the subquery; this test is here so
/// a merge that restores the panic is caught by a failing test instead of by a user.
#[tokio::test]
async fn filtering_series_by_abandoned_works_instead_of_panicking() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("abandoned_lib".to_string()),
		name: Some("Abandoned library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (given_up, given_up_books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("given_up".to_string()),
			name: Some("Given up".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;
	let (finished, finished_books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("read_through".to_string()),
			name: Some("Read through".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;

	let owner_id = app.execute_gql(r#"query { me { id } }"#, None).await["data"]["me"]
		["id"]
		.as_str()
		.expect("viewer id")
		.to_string();

	fake_data::ReadingSession {
		media_id: given_up_books[0].id.clone(),
		user_id: owner_id.clone(),
		end_percentage: 0.3,
		status: ReadingStatus::Abandoned,
		created_at: None,
	}
	.insert(app.conn())
	.await;
	fake_data::ReadingSession::completed(finished_books[0].id.clone(), owner_id.clone())
		.insert(app.conn())
		.await;

	let response = app
		.execute_gql(
			r#"query {
                series(
                    filter: { readingStatus: { is: ABANDONED } }
                    pagination: { offset: { page: 1, pageSize: 50 } }
                ) { nodes { id } }
            }"#,
			None,
		)
		.await;
	assert!(
		response.get("errors").is_none(),
		"the ABANDONED filter is broken again: {response:#}"
	);

	let ids = response["data"]["series"]["nodes"]
		.as_array()
		.expect("series list")
		.iter()
		.filter_map(|node| node["id"].as_str())
		.collect::<Vec<_>>();
	assert_eq!(
		ids,
		vec![given_up.id.as_str()],
		"the filter did not single out the abandoned series (finished: {}): {response:#}",
		finished.id
	);

	// and the negation is the complement, not another panic
	let negated = app
		.execute_gql(
			r#"query {
                series(
                    filter: { readingStatus: { isNot: ABANDONED } }
                    pagination: { offset: { page: 1, pageSize: 50 } }
                ) { nodes { id } }
            }"#,
			None,
		)
		.await;
	assert!(
		negated.get("errors").is_none(),
		"the negated ABANDONED filter is broken: {negated:#}"
	);
	let negated_ids = negated["data"]["series"]["nodes"]
		.as_array()
		.expect("series list")
		.iter()
		.filter_map(|node| node["id"].as_str())
		.collect::<Vec<_>>();
	assert!(
		negated_ids.contains(&finished.id.as_str())
			&& !negated_ids.contains(&given_up.id.as_str()),
		"isNot ABANDONED returned the wrong set: {negated:#}"
	);
}

/// A22 relies on "the latest session wins": a book picked back up is no longer abandoned. That
/// rule lives in a subquery of its own and is easy to lose.
#[tokio::test]
async fn picking_a_book_back_up_clears_the_abandoned_status() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("resumed_lib".to_string()),
		name: Some("Resumed library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (series, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("resumed".to_string()),
			name: Some("Resumed".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;

	let owner_id = app.execute_gql(r#"query { me { id } }"#, None).await["data"]["me"]
		["id"]
		.as_str()
		.expect("viewer id")
		.to_string();

	fake_data::ReadingSession {
		media_id: books[0].id.clone(),
		user_id: owner_id.clone(),
		end_percentage: 0.3,
		status: ReadingStatus::Abandoned,
		created_at: Some("2026-01-01T00:00:00Z".parse().unwrap()),
	}
	.insert(app.conn())
	.await;
	fake_data::ReadingSession {
		media_id: books[0].id.clone(),
		user_id: owner_id.clone(),
		end_percentage: 0.5,
		status: ReadingStatus::Reading,
		created_at: Some("2026-02-01T00:00:00Z".parse().unwrap()),
	}
	.insert(app.conn())
	.await;

	let response = app
		.execute_gql(
			r#"query {
                series(
                    filter: { readingStatus: { is: ABANDONED } }
                    pagination: { offset: { page: 1, pageSize: 50 } }
                ) { nodes { id } }
            }"#,
			None,
		)
		.await;
	let ids = response["data"]["series"]["nodes"]
		.as_array()
		.expect("series list")
		.iter()
		.filter_map(|node| node["id"].as_str())
		.collect::<Vec<_>>();
	assert!(
		!ids.contains(&series.id.as_str()),
		"a series being read again is still counted as abandoned: {response:#}"
	);
}
