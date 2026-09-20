use serde_json::json;

use crate::common::TestApp;

/// A1: a library may span several root folders. The extra ones are stored next to the primary
/// path and come back with the library — the scanner walks each of them.
#[tokio::test]
async fn a_library_keeps_the_extra_folders_it_was_created_with() {
	let app = TestApp::new_with_default_user().await;

	let created = app
		.execute_gql(
			r#"mutation Create($input: CreateOrUpdateLibraryInput!) {
				createLibrary(input: $input) { id name extraPaths }
			}"#,
			Some(json!({
				"input": {
					"name": "Split library",
					"path": "/tmp/np-primary",
					"extraPaths": ["/tmp/np-extra-one", "/tmp/np-extra-two"],
					"scanAfterPersist": false,
				}
			})),
		)
		.await;

	assert!(
		created.get("errors").is_none(),
		"createLibrary failed: {created:#}"
	);
	let library = &created["data"]["createLibrary"];
	let extra_paths = library["extraPaths"]
		.as_array()
		.expect("extraPaths must be returned");
	assert_eq!(
		extra_paths.len(),
		2,
		"both extra folders must be kept: {library:#}"
	);

	// and they must survive a round-trip through the query, not just the mutation's response
	let id = library["id"].as_str().expect("library id");
	let fetched = app
		.execute_gql(
			r#"query Lib($id: ID!) { libraryById(id: $id) { extraPaths } }"#,
			Some(json!({ "id": id })),
		)
		.await;
	assert_eq!(
		fetched["data"]["libraryById"]["extraPaths"]
			.as_array()
			.map(|paths| paths.len()),
		Some(2),
		"extra folders lost when reading the library back: {fetched:#}"
	);
}
