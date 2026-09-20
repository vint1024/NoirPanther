use std::path::PathBuf;

use models::entity::{media, media_metadata};
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, IntoActiveModel};
use serde_json::json;
use tests::fake_data;

use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};

/// A5: edited metadata can be written back into the EPUB itself, so the book carries it to any
/// other reader. It touches the user's files, which is exactly why it is permission-gated and
/// why the "leave a backup" flag has to work.
async fn setup_epub(app: &TestApp) -> (media::Model, PathBuf) {
	let library = fake_data::Library {
		id: Some("writeback_lib".to_string()),
		name: Some("Writeback library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (_, books) = setup_single_series_with_n_books(
		app,
		fake_data::Series {
			id: Some("writeback_series".to_string()),
			name: Some("Writeback series".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;
	let book = books.into_iter().next().expect("one book");

	// a copy of the fixture, so the test never writes into the repository's own file
	let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("../../core/integration-tests/data/book.epub")
		.canonicalize()
		.expect("book.epub fixture");
	let target = std::env::temp_dir().join(format!(
		"np-writeback-{}-{}.epub",
		std::process::id(),
		book.id
	));
	std::fs::copy(&fixture, &target).expect("copy the fixture");

	let mut active = book.clone().into_active_model();
	active.path = ActiveValue::Set(target.to_string_lossy().to_string());
	active.extension = ActiveValue::Set("epub".to_string());
	let book = active.update(app.conn()).await.expect("point at the copy");

	media_metadata::Entity::insert(media_metadata::ActiveModel {
		media_id: ActiveValue::Set(Some(book.id.clone())),
		title: ActiveValue::Set(Some("Название после правки".to_string())),
		writers: ActiveValue::Set(Some("Автор После Правки".to_string())),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("metadata insert");

	(book, target)
}

#[tokio::test]
async fn edited_metadata_is_written_into_the_book_and_can_leave_a_backup() {
	let app = TestApp::new_with_default_user().await;
	let (book, path) = setup_epub(&app).await;

	let response = app
		.execute_gql(
			r#"mutation Write($id: ID!) { writeMediaMetadataToFile(id: $id, backup: true) }"#,
			Some(json!({ "id": book.id })),
		)
		.await;
	assert!(
		response.get("errors").is_none(),
		"writeback failed: {response:#}"
	);
	assert_eq!(
		response["data"]["writeMediaMetadataToFile"], true,
		"writeback reported that it changed nothing: {response:#}"
	);

	// the file must still be a readable epub, and it must carry the new title
	use stump_core::media::processor::MediaProcessor;
	let processed = stump_core::media::processor::epub::EpubProcessor
		.process_metadata(&path)
		.expect("the written file is still a readable epub")
		.expect("it has metadata");
	assert_eq!(
		processed.title.as_deref(),
		Some("Название после правки"),
		"the new title did not reach the file"
	);

	let backup = path.with_extension("epub.bak");
	assert!(
		backup.exists(),
		"the backup flag left no copy next to {}",
		path.display()
	);

	std::fs::remove_file(&path).ok();
	std::fs::remove_file(&backup).ok();
}

#[tokio::test]
async fn writing_metadata_into_a_file_needs_the_permission() {
	let app = TestApp::new_with_default_user().await;
	let (book, path) = setup_epub(&app).await;

	let reader = CreateTestUser {
		username: "no-writeback".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			r#"mutation Write($id: ID!) { writeMediaMetadataToFile(id: $id) }"#,
			Some(json!({ "id": book.id })),
			&reader.token,
		)
		.await;

	assert!(
		response.get("errors").is_some(),
		"a user without the permission rewrote a book on disk: {response:#}"
	);

	std::fs::remove_file(&path).ok();
}
