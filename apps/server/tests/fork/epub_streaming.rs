use std::path::PathBuf;

use models::{
	entity::{content_access_rule, media, media_metadata},
	shared::enums::{ContentRuleDimension, ContentRuleMode, UserPermission},
};
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, IntoActiveModel};
use tests::fake_data;

use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};

/// Point a book at the repository's EPUB fixture. Nothing here writes to it.
async fn setup_epub_book(app: &TestApp, id: &str) -> media::Model {
	let library = fake_data::Library {
		id: Some(format!("{id}_lib")),
		name: Some(format!("{id} library")),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (_, books) = setup_single_series_with_n_books(
		app,
		fake_data::Series {
			id: Some(format!("{id}_series")),
			name: Some(format!("{id} series")),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;
	let book = books.into_iter().next().expect("one book");

	let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("../../core/integration-tests/data/book.epub")
		.canonicalize()
		.expect("book.epub fixture");

	let mut active = book.into_active_model();
	active.path = ActiveValue::Set(fixture.to_string_lossy().to_string());
	active.extension = ActiveValue::Set("epub".to_string());
	active.update(app.conn()).await.expect("point at the epub")
}

/// A6: the web and mobile readers stream an EPUB through Readium (manifest + resources) rather
/// than downloading the archive. That has to work for a reader who is NOT allowed to download
/// files — upstream has no such split, so a merge that re-gates these routes on `DownloadFile`
/// silently takes reading away from every read-only account.
#[tokio::test]
async fn a_reader_without_download_rights_can_still_stream_the_epub() {
	let app = TestApp::new_with_default_user().await;
	let book = setup_epub_book(&app, "stream").await;

	let reader = CreateTestUser {
		username: "read-only".to_string(),
		password: "password".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let manifest = app
		.server
		.get(&format!("/api/v2/epub/{}/manifest.json", book.id))
		.add_header("Authorization", format!("Bearer {}", reader.token))
		.await;
	assert_eq!(
		manifest.status_code().as_u16(),
		200,
		"a read-only user cannot open an epub: {}",
		manifest.text()
	);
	assert_eq!(
		manifest
			.headers()
			.get("content-type")
			.and_then(|value| value.to_str().ok()),
		Some("application/webpub+json"),
		"the manifest is not served as a Readium web publication"
	);
	let body: serde_json::Value = manifest.json();
	assert!(
		body["readingOrder"]
			.as_array()
			.map(|order| !order.is_empty())
			.unwrap_or(false),
		"the manifest has no reading order, so there is nothing to read: {body:#}"
	);

	// …and the whole-file download stays gated, which is the point of the split
	let download = app
		.server
		.get(&format!("/api/v2/media/{}/file", book.id))
		.add_header("Authorization", format!("Bearer {}", reader.token))
		.await;
	assert_eq!(
		download.status_code().as_u16(),
		403,
		"a user without DownloadFile got the archive itself"
	);
}

/// A6 + A3: streaming is a second door into the same books, so the content rules have to close it
/// too — otherwise a restricted user reads a hidden book by asking for its manifest.
#[tokio::test]
async fn streaming_respects_the_content_rules() {
	let app = TestApp::new_with_default_user().await;
	let book = setup_epub_book(&app, "hidden").await;

	media_metadata::Entity::insert(media_metadata::ActiveModel {
		media_id: ActiveValue::Set(Some(book.id.clone())),
		genres: ActiveValue::Set(Some("Horror".to_string())),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("metadata insert");

	let reader = CreateTestUser {
		username: "rule-bound".to_string(),
		password: "password".to_string(),
		permissions: vec![UserPermission::DownloadFile],
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

	for route in ["manifest.json", "positions.json"] {
		let response = app
			.server
			.get(&format!("/api/v2/epub/{}/{route}", book.id))
			.add_header("Authorization", format!("Bearer {}", reader.token))
			.await;
		assert_eq!(
			response.status_code().as_u16(),
			404,
			"a rule-hidden book was streamable through /{route}"
		);
	}
}
