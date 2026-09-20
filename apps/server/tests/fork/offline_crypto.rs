use std::path::PathBuf;

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use hkdf::Hkdf;
use models::{entity::media, shared::enums::UserPermission};
use p256::{
	ecdh::diffie_hellman, elliptic_curve::sec1::ToEncodedPoint, PublicKey, SecretKey,
};
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};
use serde_json::json;
use sha2::Sha256;
use tests::fake_data;

use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};

/// Must match `apps/server/src/utils/offline_crypto.rs` — and, more importantly, the Swift client
/// in the app. These constants ARE the wire format: change one and every book already downloaded
/// to a device stops opening.
const SALT: &[u8] = b"noirpanther-offline-salt-v1";
const INFO: &[u8] = b"noirpanther-offline-v1";

/// Open an AES-256-GCM combined box: nonce(12) ‖ ciphertext ‖ tag(16).
fn open(key: &[u8; 32], sealed: &[u8]) -> Vec<u8> {
	assert!(sealed.len() > 12 + 16, "box is too short to hold anything");
	let cipher = Aes256Gcm::new(key.into());
	let mut nonce_bytes = [0u8; 12];
	nonce_bytes.copy_from_slice(&sealed[..12]);
	let nonce = Nonce::from(nonce_bytes);
	cipher
		.decrypt(&nonce, &sealed[12..])
		.expect("the device could not open the box the server sealed")
}

async fn setup_book(app: &TestApp) -> (media::Model, PathBuf) {
	let library = fake_data::Library {
		id: Some("e3_lib".to_string()),
		name: Some("E3 library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (_, books) = setup_single_series_with_n_books(
		app,
		fake_data::Series {
			id: Some("e3_series".to_string()),
			name: Some("E3 series".to_string()),
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
	let book = active.update(app.conn()).await.expect("point at the epub");

	(book, fixture)
}

/// A9 (E3): the whole point of offline delivery is that the bytes leaving the server are useless
/// to everyone except the one device that asked for them. This test plays the device: it
/// generates a P-256 key, registers it, asks for the book, and then does exactly what the app's
/// Secure Enclave path does — ECDH, HKDF-SHA256, unwrap the content key, open the blob — and
/// compares the result with the file on disk.
///
/// It is also the only guard on the wire format. The salt, the info string, the 12-byte nonce
/// prefix and the X9.63 point encoding are shared with the Swift client; a "harmless" change to
/// any of them bricks every already-downloaded book, and nothing else in the suite would notice.
#[tokio::test]
async fn a_registered_device_can_decrypt_what_the_server_sends_it() {
	let app = TestApp::new_with_default_user().await;
	let (book, path) = setup_book(&app).await;

	let reader = CreateTestUser {
		username: "offline-device".to_string(),
		password: "password".to_string(),
		permissions: vec![UserPermission::OfflineRead],
		..Default::default()
	}
	.insert(&app)
	.await;

	// the "Secure Enclave" key, which in this test lives in a variable
	let device_secret = SecretKey::random(&mut aes_gcm::aead::OsRng);
	let device_public = device_secret.public_key().to_encoded_point(false);
	let b64 = base64::engine::general_purpose::STANDARD;

	let registered = app
		.server
		.post("/api/v2/media/device")
		.add_header("Authorization", format!("Bearer {}", reader.token))
		.json(&json!({
			"device_id": "test-device",
			"public_key": b64.encode(device_public.as_bytes()),
		}))
		.await;
	assert_eq!(
		registered.status_code().as_u16(),
		200,
		"registering a device key failed: {}",
		registered.text()
	);

	let response = app
		.server
		.post(&format!("/api/v2/media/{}/offline", book.id))
		.add_header("Authorization", format!("Bearer {}", reader.token))
		.json(&json!({ "device_id": "test-device" }))
		.await;
	assert_eq!(
		response.status_code().as_u16(),
		200,
		"offline delivery failed: {}",
		response.text()
	);
	let body: serde_json::Value = response.json();

	let blob = b64
		.decode(body["blob"].as_str().expect("blob"))
		.expect("blob is not base64");
	let ephemeral_pub = b64
		.decode(body["ephemeral_pub"].as_str().expect("ephemeral_pub"))
		.expect("ephemeral key is not base64");
	let wrapped_key = b64
		.decode(body["wrapped_key"].as_str().expect("wrapped_key"))
		.expect("wrapped key is not base64");

	assert_eq!(
		ephemeral_pub.len(),
		65,
		"the ephemeral key is not an uncompressed X9.63 point, which the Swift client requires"
	);

	// …and now the device side, step for step
	let ephemeral = PublicKey::from_sec1_bytes(&ephemeral_pub)
		.expect("the ephemeral key is not a P-256 point");
	let shared = diffie_hellman(device_secret.to_nonzero_scalar(), ephemeral.as_affine());
	let hkdf = Hkdf::<Sha256>::new(Some(SALT), &shared.raw_secret_bytes()[..]);
	let mut kek = [0u8; 32];
	hkdf.expand(INFO, &mut kek).expect("hkdf");

	let content_key: [u8; 32] = open(&kek, &wrapped_key)
		.try_into()
		.expect("the content key is not 32 bytes");
	let decrypted = open(&content_key, &blob);

	let original = std::fs::read(&path).expect("read the fixture");
	assert_eq!(
		decrypted.len(),
		original.len(),
		"the decrypted book is a different size than the file"
	);
	assert!(
		decrypted == original,
		"the decrypted bytes are not the book on disk"
	);
	assert_eq!(&decrypted[..2], b"PK", "the result is not even a zip/epub");

	// the blob really is encrypted — a client that skipped the unwrap would get nothing usable
	assert_ne!(&blob[..2], b"PK", "the book left the server in the clear");
}

/// A9: the wrapping is per device. Another device's key must not open the same response, which is
/// what makes "download once, share the file" useless.
#[tokio::test]
async fn another_devices_key_cannot_open_the_blob() {
	let app = TestApp::new_with_default_user().await;
	let (book, _) = setup_book(&app).await;

	let reader = CreateTestUser {
		username: "two-devices".to_string(),
		password: "password".to_string(),
		permissions: vec![UserPermission::OfflineRead],
		..Default::default()
	}
	.insert(&app)
	.await;

	let b64 = base64::engine::general_purpose::STANDARD;
	let device = SecretKey::random(&mut aes_gcm::aead::OsRng);
	app.server
		.post("/api/v2/media/device")
		.add_header("Authorization", format!("Bearer {}", reader.token))
		.json(&json!({
			"device_id": "the-phone",
			"public_key": b64.encode(device.public_key().to_encoded_point(false).as_bytes()),
		}))
		.await
		.assert_status_ok();

	let response = app
		.server
		.post(&format!("/api/v2/media/{}/offline", book.id))
		.add_header("Authorization", format!("Bearer {}", reader.token))
		.json(&json!({ "device_id": "the-phone" }))
		.await;
	let body: serde_json::Value = response.json();
	let ephemeral_pub = b64
		.decode(body["ephemeral_pub"].as_str().expect("ephemeral_pub"))
		.expect("base64");
	let wrapped_key = b64
		.decode(body["wrapped_key"].as_str().expect("wrapped_key"))
		.expect("base64");

	// a different device, with a key the server never saw
	let impostor = SecretKey::random(&mut aes_gcm::aead::OsRng);
	let ephemeral = PublicKey::from_sec1_bytes(&ephemeral_pub).expect("point");
	let shared = diffie_hellman(impostor.to_nonzero_scalar(), ephemeral.as_affine());
	let hkdf = Hkdf::<Sha256>::new(Some(SALT), &shared.raw_secret_bytes()[..]);
	let mut kek = [0u8; 32];
	hkdf.expand(INFO, &mut kek).expect("hkdf");

	let cipher = Aes256Gcm::new(&kek.into());
	let mut nonce_bytes = [0u8; 12];
	nonce_bytes.copy_from_slice(&wrapped_key[..12]);
	let nonce = Nonce::from(nonce_bytes);
	assert!(
		cipher.decrypt(&nonce, &wrapped_key[12..]).is_err(),
		"a device the book was not wrapped for recovered the content key"
	);
}
