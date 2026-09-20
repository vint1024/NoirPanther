use models::entity::media;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use tests::fake_data;

use crate::common::{series::setup_single_series_with_n_books, TestApp};

/// A2: folders that hold one work split across several directories can be merged into a single
/// series, and the merge can be undone. The books move; the folders on disk are untouched, and a
/// later scan keeps sending them to the target (that mapping is what `mergedSources` records).
#[tokio::test]
async fn series_can_be_merged_and_unmerged() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("merge_lib".to_string()),
		name: Some("Merge library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (target, _) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("merge_target".to_string()),
			name: Some("Target".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;
	let (source, source_books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("merge_source".to_string()),
			name: Some("Source".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		2,
	)
	.await;

	let merged = app
		.execute_gql(
			r#"mutation Merge($targetId: ID!, $sourceIds: [ID!]!) {
				mergeSeries(targetId: $targetId, sourceIds: $sourceIds) {
					id
					mergedSources { path }
				}
			}"#,
			Some(json!({ "targetId": target.id, "sourceIds": [source.id] })),
		)
		.await;
	assert!(
		merged.get("errors").is_none(),
		"mergeSeries failed: {merged:#}"
	);
	assert_eq!(
		merged["data"]["mergeSeries"]["mergedSources"]
			.as_array()
			.map(|sources| sources.len()),
		Some(1),
		"the merged folder must be recorded: {merged:#}"
	);

	// the books themselves moved to the target series
	for book in &source_books {
		let moved = media::Entity::find()
			.filter(media::Column::Id.eq(book.id.clone()))
			.one(app.conn())
			.await
			.expect("query")
			.expect("book still exists");
		assert_eq!(
			moved.series_id.as_deref(),
			Some(target.id.as_str()),
			"book {} did not move into the target series",
			book.name
		);
	}

	let unmerged = app
		.execute_gql(
			r#"mutation Unmerge($id: ID!) { unmergeSeries(id: $id) { id mergedSources { path } } }"#,
			// the merge is undone from the series that absorbed the folders
			Some(json!({ "id": target.id })),
		)
		.await;
	assert!(
		unmerged.get("errors").is_none(),
		"unmergeSeries failed: {unmerged:#}"
	);

	let target_after = app
		.execute_gql(
			r#"query Series($id: ID!) { seriesById(id: $id) { mergedSources { path } } }"#,
			Some(json!({ "id": target.id })),
		)
		.await;
	assert_eq!(
		target_after["data"]["seriesById"]["mergedSources"]
			.as_array()
			.map(|sources| sources.len()),
		Some(0),
		"unmerge must drop the recorded folder: {target_after:#}"
	);
}
