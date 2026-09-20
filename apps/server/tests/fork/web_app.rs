use crate::common::TestApp;

/// A23/B2: the web app installs to a phone's home screen. Upstream has no route for the
/// manifest, so it fell through to the SPA fallback and came back as index.html — iOS then
/// showed a letter icon and the page title instead of the app's name and panther icon.
#[tokio::test]
async fn the_web_manifest_is_served_as_a_manifest() {
	let app = TestApp::new().await;

	let response = app.get("/manifest.webmanifest").await;
	let content_type = response
		.headers()
		.get("content-type")
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default();

	assert!(
		content_type.starts_with("application/manifest+json"),
		"the manifest must not fall through to the SPA: content-type was `{content_type}`"
	);
}

/// A25: releases are `<Stump version>-r<N>`, and the server tells the owner when a newer one
/// exists. The endpoint has to stay reachable (and keep its name) for that notice to work.
#[tokio::test]
async fn the_update_check_endpoint_answers() {
	let app = TestApp::new_with_default_user().await;

	let response = app.get("/api/v2/check-for-update").await;
	assert!(
		response.status_code().is_success(),
		"update check is unreachable: {}",
		response.status_code()
	);
}
