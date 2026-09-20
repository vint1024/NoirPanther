use chrono::{Duration, Utc};
use models::entity::session;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, IntoActiveModel, QueryOrder};
use serde_json::json;

use crate::common::TestApp;

const USERNAME: &str = "initial-server-admin";
const PASSWORD: &str = "password";

async fn login_with(app: &TestApp, query: &str) -> String {
	let response = app
		.server
		.post(&format!("/api/v2/auth/login{query}"))
		.json(&json!({ "username": USERNAME, "password": PASSWORD }))
		.await;
	response.assert_status_ok();
	response
		.headers()
		.get_all("set-cookie")
		.into_iter()
		.filter_map(|value| value.to_str().ok())
		.find(|value| value.starts_with("stump_session="))
		.unwrap_or_else(|| panic!("login set no session cookie"))
		.to_string()
}

/// the `name=value` pair on its own, which is what a client sends back
fn cookie_pair(set_cookie: &str) -> String {
	set_cookie
		.split(';')
		.next()
		.expect("a set-cookie header is never empty")
		.to_string()
}

async fn latest_session(app: &TestApp) -> session::Model {
	session::Entity::find()
		.order_by_desc(session::Column::Id)
		.one(app.conn())
		.await
		.expect("session query")
		.expect("login created no session row")
}

/// A13: the auth middleware only reads sessions, so tower-sessions never re-saves them and the
/// expiry froze at login+TTL — every web client was logged out on a fixed schedule regardless of
/// activity. We slide the row forward on activity instead. The slide is fire-and-forget from the
/// middleware, which is exactly why it can disappear in a merge without anything failing.
#[tokio::test]
async fn activity_slides_a_live_session_forward() {
	let app = TestApp::new().await;
	app.create_initial_account().await;

	let cookie = login_with(&app, "").await;
	let created = latest_session(&app).await;

	// pretend the session was created a while ago: far enough back to pass the one-hour
	// throttle, close enough that it is still valid
	let stale = Utc::now() + Duration::hours(12);
	let mut active = created.clone().into_active_model();
	active.expiry_time = ActiveValue::Set(stale.into());
	let stale_row = active
		.update(app.conn())
		.await
		.expect("could not age the session");

	app.server
		.get("/api/v2/auth/me")
		.add_header("Cookie", cookie_pair(&cookie))
		.await
		.assert_status_ok();

	// the slide runs in a spawned task, so give it a moment to land
	let mut slid = None;
	for _ in 0..40 {
		tokio::time::sleep(std::time::Duration::from_millis(50)).await;
		let row = session::Entity::find_by_id(stale_row.id)
			.one(app.conn())
			.await
			.expect("session query")
			.expect("the session row vanished");
		if row.expiry_time > stale_row.expiry_time {
			slid = Some(row);
			break;
		}
	}
	let slid = slid.expect("an authenticated request did not slide the session expiry");
	assert!(
		slid.expiry_time > DateTimeWithTimeZone::from(Utc::now() + Duration::days(2)),
		"the session was slid to {}, which is not a fresh 3-day window",
		slid.expiry_time
	);
}

/// A13: sliding must not resurrect a session that has already run out — that would make the
/// window infinite instead of an inactivity window.
#[tokio::test]
async fn an_expired_session_is_not_revived_by_a_request() {
	let app = TestApp::new().await;
	app.create_initial_account().await;

	let cookie = login_with(&app, "").await;
	let created = latest_session(&app).await;

	let mut active = created.clone().into_active_model();
	active.expiry_time = ActiveValue::Set((Utc::now() - Duration::hours(1)).into());
	let expired = active
		.update(app.conn())
		.await
		.expect("could not expire the session");

	let response = app
		.server
		.get("/api/v2/auth/me")
		.add_header("Cookie", cookie_pair(&cookie))
		.await;
	assert_eq!(
		response.status_code().as_u16(),
		401,
		"an expired session still authenticated a request"
	);

	tokio::time::sleep(std::time::Duration::from_millis(200)).await;
	let row = session::Entity::find_by_id(expired.id)
		.one(app.conn())
		.await
		.expect("session query")
		.expect("the session row vanished");
	assert_eq!(
		row.expiry_time, expired.expiry_time,
		"an expired session was slid back to life"
	);
}

/// B14: "Remember me" asks for a persistent cookie and a 30-day session. Without it the cookie
/// must stay a browser-session cookie — the iOS PWA keeps the one and drops the other, so the
/// difference is the whole feature.
#[tokio::test]
async fn remember_me_makes_the_session_and_its_cookie_persistent() {
	let app = TestApp::new().await;
	app.create_initial_account().await;

	let plain_cookie = login_with(&app, "").await;
	let plain_session = latest_session(&app).await;
	assert!(
		!plain_cookie.to_lowercase().contains("expires=")
			&& !plain_cookie.to_lowercase().contains("max-age="),
		"an ordinary login handed out a persistent cookie: {plain_cookie}"
	);
	assert!(
		plain_session.expiry_time
			< DateTimeWithTimeZone::from(Utc::now() + Duration::days(7)),
		"an ordinary login got the long session TTL: {}",
		plain_session.expiry_time
	);

	let remembered_cookie = login_with(&app, "?remember=true").await;
	let remembered = latest_session(&app).await;
	let lowered = remembered_cookie.to_lowercase();
	assert!(
		lowered.contains("max-age=") || lowered.contains("expires="),
		"remember me left a session cookie, which the PWA drops: {remembered_cookie}"
	);
	if let Some(max_age) = lowered
		.split("max-age=")
		.nth(1)
		.and_then(|rest| rest.split(';').next())
		.and_then(|value| value.trim().parse::<i64>().ok())
	{
		assert!(
			max_age > 29 * 24 * 60 * 60,
			"the remembered cookie only lives {max_age} seconds, not 30 days"
		);
	}
	assert!(
		remembered.expiry_time
			> DateTimeWithTimeZone::from(Utc::now() + Duration::days(29)),
		"remember me did not give the session its 30-day TTL: {}",
		remembered.expiry_time
	);
	assert!(
		remembered.id != plain_session.id,
		"the two logins reused one session row, so the comparison proves nothing"
	);

	// and it actually works: the remembered cookie authenticates
	app.server
		.get("/api/v2/auth/me")
		.add_header("Cookie", cookie_pair(&remembered_cookie))
		.await
		.assert_status_ok();
}
