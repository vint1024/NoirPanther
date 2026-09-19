pub(crate) mod auth;
pub(crate) mod emoji;
pub(crate) mod epub;
pub(crate) mod library;
pub(crate) mod media;
mod oidc;
mod series;
mod user;

use axum::{
	extract::State,
	http::StatusCode,
	response::IntoResponse,
	routing::{get, post},
	Json, Router,
};
use models::entity;
use reqwest::header::USER_AGENT;
use sea_orm::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
	config::state::AppState,
	errors::{APIError, APIResult},
};

pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	Router::new()
		.merge(auth::mount(app_state.clone()))
		.merge(oidc::mount())
		.merge(emoji::mount(app_state.clone()))
		.merge(media::mount(app_state.clone()))
		.merge(epub::mount(app_state.clone()))
		.merge(series::mount(app_state.clone()))
		.merge(library::mount(app_state.clone()))
		.merge(user::mount(app_state))
		.route("/claim", get(claim))
		.route("/health", get(health))
		.route("/ping", get(ping))
		.route("/version", post(version))
		.route("/check-for-update", get(check_for_updates))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimResponse {
	pub is_claimed: bool,
}

async fn claim(State(ctx): State<AppState>) -> APIResult<Json<ClaimResponse>> {
	let is_claimed = entity::user::Entity::find()
		.count(ctx.conn.as_ref())
		.await?
		> 0;

	Ok(Json(ClaimResponse { is_claimed }))
}

async fn ping() -> APIResult<String> {
	Ok("pong".to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StumpVersion {
	pub semver: String,
	// E.g., nightly, experimental, unstable, etc.
	pub build_channel: Option<String>,
	pub rev: String,
	pub compile_time: String,
}

async fn version() -> APIResult<Json<StumpVersion>> {
	Ok(Json(StumpVersion {
		semver: env!("CARGO_PKG_VERSION").to_string(),
		build_channel: option_env!("BUILD_CHANNEL").map(|s| s.to_string()),
		rev: env!("GIT_REV").to_string(),
		compile_time: env!("STATIC_BUILD_DATE").to_string(),
	}))
}

/// The GitHub repository whose releases this build is checked against. NoirPanther
/// releases are tagged `v<upstream semver>-r<N>` (e.g. `v0.1.7-r2`): the semver is the
/// Stump release the build is based on, `rN` is the fork revision on top of it.
const RELEASES_REPO: &str = "vint1024/NoirPanther";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
	current_semver: String,
	latest_semver: String,
	has_update_available: bool,
	/// Link to the latest release page, when one is known
	release_url: Option<String>,
}

/// Parses `X.Y.Z` or `X.Y.Z-rN` (with or without a leading `v`) into a tuple that
/// orders the way releases do: upstream version first, then the fork revision
/// (a bare `X.Y.Z` counts as revision 0).
fn parse_release_version(version: &str) -> Option<(u64, u64, u64, u64)> {
	let version = version.trim().trim_start_matches('v');
	let (base, revision) = match version.split_once('-') {
		Some((base, suffix)) => (base, suffix.strip_prefix('r')?.parse().ok()?),
		None => (version, 0),
	};
	let mut parts = base.split('.').map(|part| part.parse::<u64>().ok());
	let parsed = (parts.next()??, parts.next()??, parts.next()??, revision);
	parts.next().is_none().then_some(parsed)
}

/// Whether `latest` is a newer release than `current`. Versions that don't follow the
/// release scheme (local/dev builds) fall back to a plain inequality check.
fn is_newer_release(current: &str, latest: &str) -> bool {
	match (
		parse_release_version(current),
		parse_release_version(latest),
	) {
		(Some(current), Some(latest)) => latest > current,
		_ => latest.trim_start_matches('v') != current.trim_start_matches('v'),
	}
}

async fn check_for_updates() -> APIResult<Json<UpdateCheck>> {
	let current_semver = env!("CARGO_PKG_VERSION").to_string();

	let client = reqwest::Client::new();
	let github_response = client
		.get(format!(
			"https://api.github.com/repos/{RELEASES_REPO}/releases/latest"
		))
		.header(USER_AGENT, RELEASES_REPO)
		.send()
		.await?;

	if github_response.status().is_success() {
		let github_json: serde_json::Value = github_response.json().await?;

		let latest_semver = github_json["tag_name"]
			.as_str()
			.ok_or_else(|| {
				APIError::InternalServerError(
					"Failed to parse latest release tag name".to_string(),
				)
			})?
			.trim_start_matches('v')
			.to_string();

		let has_update_available = is_newer_release(&current_semver, &latest_semver);

		Ok(Json(UpdateCheck {
			current_semver,
			latest_semver,
			has_update_available,
			release_url: github_json["html_url"].as_str().map(str::to_string),
		}))
	} else {
		match github_response.status().as_u16() {
			// No releases yet, or GitHub rate limited this address: not an error worth
			// surfacing in the settings page
			403 | 404 | 429 => Ok(Json(UpdateCheck {
				current_semver,
				latest_semver: "unknown".to_string(),
				has_update_available: false,
				release_url: None,
			})),
			_ => Err(APIError::InternalServerError(format!(
				"Failed to fetch latest release: {}",
				github_response.status()
			))),
		}
	}
}

async fn health(State(ctx): State<AppState>) -> impl IntoResponse {
	let ok_status = json!({"status": "ok"});

	let (db_ready, db_data) = match ctx.conn.ping().await {
		Ok(_) => (true, ok_status.clone()),
		Err(e) => (false, json!({"status": "error", "message": e.to_string()})),
	};

	let (spa_available, spa_data) =
		match tokio::fs::metadata(&ctx.config.client_dir).await {
			Ok(metadata) if metadata.is_dir() => (true, ok_status),
			Ok(_) => (
				false,
				json!({"status": "error", "message": "The client directory is malformed or missing"}),
			),
			Err(e) => (false, json!({"status": "error", "message": e.to_string()})),
		};

	let status_code = if [db_ready, spa_available].iter().all(|&ready| ready) {
		StatusCode::OK
	} else {
		StatusCode::SERVICE_UNAVAILABLE
	};
	let payload = json!({
		"status": if status_code == StatusCode::OK { "ok" } else { "error" },
		"dependencies": {
			"database": db_data,
			"spa": spa_data
		}
	});

	// ^ the above structure is pretty overkill for two dependencies, but this is how
	// i've done it in the past (at least when i don't need background periodic checks or
	// checks against external deps) and will make it easier to add more down the
	// road if needed

	(status_code, Json(payload))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_release_versions() {
		assert_eq!(parse_release_version("0.1.7"), Some((0, 1, 7, 0)));
		assert_eq!(parse_release_version("v0.1.7-r2"), Some((0, 1, 7, 2)));
		assert_eq!(parse_release_version("0.1.7-r12"), Some((0, 1, 7, 12)));
		assert_eq!(parse_release_version("0.1.7-vint-0.5.0"), None);
		assert_eq!(parse_release_version("0.1"), None);
		assert_eq!(parse_release_version("0.1.7.1"), None);
	}

	#[test]
	fn compares_releases() {
		assert!(is_newer_release("0.1.7-r1", "0.1.7-r2"));
		assert!(is_newer_release("0.1.7-r9", "0.1.7-r10"));
		assert!(is_newer_release("0.1.7-r5", "0.1.8-r1"));
		assert!(!is_newer_release("0.1.7-r2", "0.1.7-r2"));
		assert!(!is_newer_release("0.1.7-r2", "v0.1.7-r2"));
		// A server built ahead of the published release is not "out of date"
		assert!(!is_newer_release("0.1.7-r3", "0.1.7-r2"));
		// Builds outside the scheme: anything different counts as an update
		assert!(is_newer_release("0.1.7-vint-0.5.0", "0.1.7-r1"));
	}
}
