use models::{
	entity::{content_access_rule, media_metadata},
	shared::enums::{ContentRuleDimension, ContentRuleMode, UserPermission},
};
use sea_orm::{ActiveValue, EntityTrait};
use serde_json::json;
use tests::fake_data;

use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};

/// A9: `POST /media/{id}/offline` hands a device an encrypted copy of a book. It is a third door
/// into the library — next to the listing and to streaming — and it has its own permission,
/// `OfflineRead`, precisely so it can be granted without handing out the plaintext file. These
/// tests pin the gates only. The E3 crypto itself (ECDH to the device key, AES-256-GCM over the
/// book) has no test at all yet — worth one.
async fn setup_book(app: &TestApp) -> models::entity::media::Model {
	let library = fake_data::Library {
		id: Some("offline_lib".to_string()),
		name: Some("Offline library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (_, books) = setup_single_series_with_n_books(
		app,
		fake_data::Series {
			id: Some("offline_series".to_string()),
			name: Some("Offline series".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;
	books.into_iter().next().expect("one book")
}

#[tokio::test]
async fn offline_download_needs_its_own_permission() {
	let app = TestApp::new_with_default_user().await;
	let book = setup_book(&app).await;

	// even DownloadFile is not it: the two are deliberately separate
	let reader = CreateTestUser {
		username: "no-offline".to_string(),
		password: "password".to_string(),
		permissions: vec![UserPermission::DownloadFile],
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.server
		.post(&format!("/api/v2/media/{}/offline", book.id))
		.add_header("Authorization", format!("Bearer {}", reader.token))
		.json(&json!({ "device_id": "some-device" }))
		.await;
	assert_eq!(
		response.status_code().as_u16(),
		403,
		"a user without OfflineRead was let into the offline endpoint"
	);
}

#[tokio::test]
async fn offline_download_respects_the_content_rules() {
	let app = TestApp::new_with_default_user().await;
	let book = setup_book(&app).await;

	media_metadata::Entity::insert(media_metadata::ActiveModel {
		media_id: ActiveValue::Set(Some(book.id.clone())),
		genres: ActiveValue::Set(Some("Horror".to_string())),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("metadata insert");

	let reader = CreateTestUser {
		username: "offline-reader".to_string(),
		password: "password".to_string(),
		permissions: vec![UserPermission::OfflineRead],
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

	let response = app
		.server
		.post(&format!("/api/v2/media/{}/offline", book.id))
		.add_header("Authorization", format!("Bearer {}", reader.token))
		.json(&json!({ "device_id": "some-device" }))
		.await;
	assert_eq!(
		response.status_code().as_u16(),
		404,
		"a rule-hidden book could be taken offline"
	);
}
