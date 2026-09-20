use crate::common::TestApp;

/// The web app is served without any security headers upstream. Book descriptions are rendered
/// as HTML (they come out of files downloaded from the internet), so the page needs a policy
/// that stops an injected `<iframe>`, off-site `<form>` or third-party image from doing
/// anything, and stops the app itself from being framed.
#[tokio::test]
async fn web_app_is_served_with_a_content_security_policy() {
	let app = TestApp::new().await;
	let response = app.get("/").await;

	let csp = response
		.headers()
		.get("content-security-policy")
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default()
		.to_string();

	assert!(!csp.is_empty(), "the web app must carry a CSP");
	for directive in [
		"frame-ancestors 'none'", // nobody may frame the app (clickjacking)
		"form-action 'self'",     // an injected form cannot post off-site (phishing)
		// the EPUB reader frames its own blob: content, so only off-site frames are blocked
		"frame-src 'self' blob:",
		"object-src 'none'",          // no plugins
		"base-uri 'self'",            // no rewriting of relative URLs
		"img-src 'self' data: blob:", // no third-party tracking pixels
	] {
		assert!(
			csp.contains(directive),
			"CSP is missing `{directive}`; it is: {csp}"
		);
	}
}

#[tokio::test]
async fn web_app_refuses_content_type_sniffing_and_leaks_no_referrer() {
	let app = TestApp::new().await;
	let response = app.get("/").await;
	let header = |name: &str| {
		response
			.headers()
			.get(name)
			.and_then(|value| value.to_str().ok())
			.unwrap_or_default()
			.to_string()
	};

	assert_eq!(header("x-content-type-options"), "nosniff");
	assert_eq!(header("referrer-policy"), "strict-origin-when-cross-origin");
}

/// The session cookie is what authenticates the web app, so it must never be readable from
/// JavaScript, and must not ride along on cross-site requests.
#[tokio::test]
async fn session_cookie_is_http_only_and_same_site() {
	let app = TestApp::new().await;
	app.create_initial_account().await;

	let response = app
		.server
		.post("/api/v2/auth/login")
		.json(&serde_json::json!({
			"username": "initial-server-admin",
			"password": "password",
		}))
		.await;

	let cookie = response
		.headers()
		.get("set-cookie")
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default()
		.to_string();

	assert!(
		cookie.contains("stump_session="),
		"login must set the session cookie, got: {cookie}"
	);
	assert!(
		cookie.contains("HttpOnly"),
		"cookie must be HttpOnly: {cookie}"
	);
	assert!(
		cookie.contains("SameSite=Lax"),
		"cookie must be SameSite: {cookie}"
	);
}
