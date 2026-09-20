use serde_json::json;

use crate::common::TestApp;

const PASSWORD: &str = "password";
const USERNAME: &str = "initial-server-admin";

async fn attempt(app: &TestApp, password: &str) -> u16 {
	app.server
		.post("/api/v2/auth/login")
		.json(&json!({ "username": USERNAME, "password": password }))
		.await
		.status_code()
		.as_u16()
}

/// Upstream counts a user's failed sign-ins over 24 hours from anywhere and, at nine, locks the
/// account until an administrator unlocks it by hand — so anyone who knows a username can lock
/// its owner out, the server owner included. This fork refuses the *address* for a short window
/// instead and never touches the account.
#[tokio::test]
async fn repeated_failures_throttle_the_address_without_locking_the_account() {
	// new_with_default_user keeps the owner's token, so the account can be inspected afterwards
	let app = TestApp::new_with_default_user().await;

	for i in 1..=9 {
		let status = attempt(&app, "wrong-password").await;
		assert_eq!(status, 401, "attempt {i} should simply be rejected");
	}

	// the tenth attempt from this address is refused outright…
	assert_eq!(
		attempt(&app, "wrong-password").await,
		429,
		"a run of failures must throttle the address"
	);

	// …including one carrying the right password, because the address is what is blocked
	assert_eq!(
		attempt(&app, PASSWORD).await,
		429,
		"the throttle applies to the address, not to the credentials"
	);

	// and the account itself must be untouched: no administrator has to unlock anything
	let locked = app
		.execute_gql(
			r#"query { users(pagination: { offset: { page: 1, pageSize: 10 } }) { nodes { username isLocked } } }"#,
			None,
		)
		.await;
	let is_locked = locked["data"]["users"]["nodes"]
		.as_array()
		.and_then(|nodes| {
			nodes
				.iter()
				.find(|node| node["username"] == USERNAME)
				.and_then(|node| node["isLocked"].as_bool())
		});
	assert_eq!(
		is_locked,
		Some(false),
		"the account must not be locked by failed sign-ins: {locked:#}"
	);
}
