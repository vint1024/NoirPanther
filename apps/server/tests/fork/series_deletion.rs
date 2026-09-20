use models::entity::{media, series, series_merge};
use sea_orm::{ActiveValue, EntityTrait, PaginatorTrait};
use serde_json::json;
use tests::fake_data;

use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};

/// A16: upstream can only clean a whole library; this fork can delete one series — the way out
/// when a single folder is gone and re-scanning the rest is not wanted. It deletes rows, never
/// files, and it has to take the books with it (an orphaned book is invisible and undeletable)
/// and drop the merge entries that fed it (a later scan would otherwise route books into a series
/// that no longer exists).
#[tokio::test]
async fn deleting_a_series_takes_its_books_and_merge_entries_with_it() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("deletion_lib".to_string()),
		name: Some("Deletion library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (doomed, doomed_books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("doomed".to_string()),
			name: Some("Doomed".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		2,
	)
	.await;
	let (spared, _) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("spared".to_string()),
			name: Some("Spared".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;

	// something was merged into the doomed series earlier
	series_merge::Entity::insert(series_merge::ActiveModel {
		source_path: ActiveValue::Set("/books/old-folder".to_string()),
		target_series_id: ActiveValue::Set(doomed.id.clone()),
		source_name: ActiveValue::Set("Old folder".to_string()),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("merge entry insert");

	let response = app
		.execute_gql(
			r#"mutation Delete($id: ID!) { deleteSeries(id: $id) }"#,
			Some(json!({ "id": doomed.id })),
		)
		.await;
	assert!(
		response.get("errors").is_none(),
		"deleting the series failed: {response:#}"
	);

	assert!(
		series::Entity::find_by_id(doomed.id.clone())
			.one(app.conn())
			.await
			.expect("series query")
			.is_none(),
		"the series is still there"
	);
	for book in &doomed_books {
		assert!(
			media::Entity::find_by_id(book.id.clone())
				.one(app.conn())
				.await
				.expect("media query")
				.is_none(),
			"a book of the deleted series outlived it: {}",
			book.id
		);
	}
	assert_eq!(
		series_merge::Entity::find()
			.count(app.conn())
			.await
			.expect("merge query"),
		0,
		"a merge entry still points at the deleted series"
	);

	// the neighbour is untouched
	assert!(
		series::Entity::find_by_id(spared.id)
			.one(app.conn())
			.await
			.expect("series query")
			.is_some(),
		"deleting one series took another with it"
	);
}

/// A16: it is a destructive, library-wide capability, so it sits behind ManageLibrary — a plain
/// reader must not be able to erase a series from the database.
#[tokio::test]
async fn deleting_a_series_requires_managing_libraries() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("guard_lib".to_string()),
		name: Some("Guard library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let (series, _) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("guarded".to_string()),
			name: Some("Guarded".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;

	let reader = CreateTestUser {
		username: "just-a-reader".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			r#"mutation Delete($id: ID!) { deleteSeries(id: $id) }"#,
			Some(json!({ "id": series.id })),
			&reader.token,
		)
		.await;
	assert!(
		response.get("errors").is_some(),
		"a reader deleted a series: {response:#}"
	);
	assert!(
		series::Entity::find_by_id(series.id)
			.one(app.conn())
			.await
			.expect("series query")
			.is_some(),
		"the series was deleted despite the error"
	);
}

/// A8: the series page has its own "regenerate thumbnail" action — upstream can only sweep a
/// whole library. It is gated on EditThumbnails and, like everything else, on being able to see
/// the series at all.
#[tokio::test]
async fn regenerating_one_series_thumbnail_is_gated() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("thumb_lib".to_string()),
		name: Some("Thumbnail library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let (series, _) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("thumbed".to_string()),
			name: Some("Thumbed".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;

	let reader = CreateTestUser {
		username: "no-thumbnails".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let refused = app
		.execute_gql_with_token(
			r#"mutation Regenerate($id: ID!) { generateSeriesThumbnail(id: $id) }"#,
			Some(json!({ "id": series.id })),
			&reader.token,
		)
		.await;
	assert!(
		refused.get("errors").is_some(),
		"a user without EditThumbnails started a thumbnail job: {refused:#}"
	);

	// the owner may, and the mutation still exists under this name (the app calls it)
	let allowed = app
		.execute_gql(
			r#"mutation Regenerate($id: ID!) { generateSeriesThumbnail(id: $id) }"#,
			Some(json!({ "id": series.id })),
		)
		.await;
	assert!(
		allowed.get("errors").is_none(),
		"the owner could not regenerate a series thumbnail: {allowed:#}"
	);
}
