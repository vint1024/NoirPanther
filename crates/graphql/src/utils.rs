use std::path::Path;

use models::entity::user::AuthUser;
use sea_orm::{
	prelude::DateTimeWithTimeZone, ConnectionTrait, DatabaseConnection, Statement, Value,
};
use stump_core::filesystem::find_thumbnail;
use tower_sessions::Session;

pub async fn save_user_session(session: &Session, user: AuthUser) {
	if let Err(error) = session.insert("user", user).await {
		tracing::error!(?error, "Failed to save user session");
	}
}

/// Build a raw SQL [`Statement`] using the actual database backend of `conn`.
/// Write SQL with `$1`, `$2`, … placeholders — they work for both PostgreSQL
/// and SQLite (SQLite treats them as named parameters).
pub fn db_statement(
	conn: &DatabaseConnection,
	sql: impl Into<String>,
	values: impl IntoIterator<Item = Value>,
) -> Statement {
	Statement::from_sql_and_values(conn.get_database_backend(), sql, values)
}

/// A cache-busting version for a thumbnail URL. Thumbnail responses are served
/// with a one-year `Cache-Control`, so clients (the web app, NoirPanther's
/// image cache) only refetch a cover when its URL changes. The version is the
/// modification time of the generated thumbnail file when one exists (it is
/// rewritten whenever thumbnails are regenerated), otherwise `fallback` — the
/// entity's own timestamp (a library's last scan, a series'/book's update).
pub async fn thumbnail_version(
	thumbnails_dir: &Path,
	id: &str,
	fallback: Option<DateTimeWithTimeZone>,
) -> Option<i64> {
	if let Some(path) = find_thumbnail(thumbnails_dir, id).await {
		if let Ok(modified) = tokio::fs::metadata(&path)
			.await
			.and_then(|meta| meta.modified())
		{
			if let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) {
				return Some(since_epoch.as_secs() as i64);
			}
		}
	}
	fallback.map(|ts| ts.timestamp())
}

/// Append `?v=<version>` to a URL (no-op when there is no version)
pub fn versioned_url(url: String, version: Option<i64>) -> String {
	match version {
		Some(v) => format!("{url}?v={v}"),
		None => url,
	}
}
