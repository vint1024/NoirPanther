use migrations::{Migrator, MigratorTrait};
use models::entity::{
	age_restriction, api_key, book_club, book_club_book, book_club_discussion,
	book_club_member, content_access_rule, device_public_key, kobo_sync_session, library,
	library_config, library_exclusion, library_path, media, media_analysis,
	media_metadata, media_tag, reading_device, reading_session, refresh_token, series,
	series_merge, series_metadata, server_config, session, tag, user,
	user_login_activity, user_preferences,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, DbConn, DbErr, Schema};

/// Point this at a PostgreSQL server to run the suite against it instead of in-memory SQLite:
///
/// ```sh
/// TEST_DATABASE_URL=postgresql://stump:stump-local@localhost:15432/stump \
///   cargo test --workspace -- --test-threads=1
/// ```
///
/// It matters because production runs PostgreSQL and the suite otherwise only ever exercises
/// SQLite — the very fixes that exist *because* Postgres behaves differently (A17/A18/A19 in
/// FORK_BACKLOG) were tested by hand and by nothing else.
pub const TEST_DATABASE_URL: &str = "TEST_DATABASE_URL";

/// How long a leftover test database may live before another run reclaims it. Databases are
/// named `stump_test_<unix seconds>_<random>`, so cleanup needs no bookkeeping and never touches
/// one a parallel run is still using.
const STALE_TEST_DB_SECS: u64 = 30 * 60;

pub async fn test_database() -> DbConn {
	match std::env::var(TEST_DATABASE_URL) {
		Ok(url) if url.starts_with("postgres") => postgres_test_database(&url).await,
		_ => {
			// Connect through models::db so custom SQL functions (ulower) exist in
			// test databases too — content-rule queries reference them
			let db = models::db::connect_sqlite("sqlite::memory:")
				.await
				.expect("failed to connect to test database");

			create_database_tables(&db)
				.await
				.expect("failed to create test database tables");

			db
		},
	}
}

/// Create a throwaway database on the given server, migrate it, and hand back a connection.
///
/// The schema comes from the real migrations rather than from the entities, so this also checks
/// that the migrations themselves apply on PostgreSQL.
async fn postgres_test_database(admin_url: &str) -> DbConn {
	let admin = Database::connect(admin_url)
		.await
		.expect("failed to connect to the PostgreSQL server named by TEST_DATABASE_URL");

	drop_stale_test_databases(&admin).await;

	let name = format!(
		"stump_test_{}_{}",
		std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.expect("the clock is before 1970")
			.as_secs(),
		uuid::Uuid::new_v4().simple()
	);
	admin
		.execute_unprepared(&format!("CREATE DATABASE {name}"))
		.await
		.unwrap_or_else(|error| panic!("failed to create {name}: {error}"));

	let db = Database::connect(swap_database(admin_url, &name))
		.await
		.unwrap_or_else(|error| panic!("failed to connect to {name}: {error}"));

	// The fork's filters call ulower(); on PostgreSQL the server defines it as a wrapper over
	// lower() at startup (see core::database::connect), so a test database needs it too.
	db.execute_unprepared(&format!(
		"CREATE OR REPLACE FUNCTION {fn}(text) RETURNS text AS $$ SELECT lower($1) $$ LANGUAGE SQL IMMUTABLE PARALLEL SAFE STRICT",
		fn = models::db::UNICODE_LOWER_FN
	))
	.await
	.expect("failed to define ulower()");

	Migrator::up(&db, None)
		.await
		.expect("migrations failed on PostgreSQL");

	db
}

/// Replace the database name in a PostgreSQL URL, keeping credentials, host and query intact.
fn swap_database(url: &str, name: &str) -> String {
	let (head, tail) = match url.split_once("://") {
		Some((scheme, rest)) => (scheme, rest),
		None => panic!("TEST_DATABASE_URL is not a URL: {url}"),
	};
	let (authority, rest) = tail.split_once('/').unwrap_or((tail, ""));
	let query = rest.split_once('?').map(|(_, q)| q);
	match query {
		Some(query) => format!("{head}://{authority}/{name}?{query}"),
		None => format!("{head}://{authority}/{name}"),
	}
}

/// Drop test databases an earlier run left behind. Anything younger than [`STALE_TEST_DB_SECS`]
/// is left alone so a parallel run's databases survive.
async fn drop_stale_test_databases(admin: &DbConn) {
	let Ok(rows) = admin
		.query_all(sea_orm::Statement::from_string(
			DbBackend::Postgres,
			"SELECT datname FROM pg_database WHERE datname LIKE 'stump_test_%'",
		))
		.await
	else {
		return;
	};

	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0);

	for row in rows {
		let Ok(name) = row.try_get::<String>("", "datname") else {
			continue;
		};
		let created = name
			.strip_prefix("stump_test_")
			.and_then(|rest| rest.split('_').next())
			.and_then(|secs| secs.parse::<u64>().ok())
			.unwrap_or(0);
		if now.saturating_sub(created) < STALE_TEST_DB_SECS {
			continue;
		}
		// Best effort: a database still in use simply stays until the next run.
		let _ = admin
			.execute_unprepared(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
			.await;
	}
}

pub async fn create_database_tables(db: &DbConn) -> Result<(), DbErr> {
	let schema = Schema::new(DbBackend::Sqlite);

	let tables = [
		schema.create_table_from_entity(api_key::Entity),
		schema.create_table_from_entity(media::Entity),
		schema.create_table_from_entity(media_metadata::Entity),
		schema.create_table_from_entity(media_analysis::Entity),
		schema.create_table_from_entity(series::Entity),
		schema.create_table_from_entity(series_metadata::Entity),
		schema.create_table_from_entity(library_exclusion::Entity),
		schema.create_table_from_entity(kobo_sync_session::Entity),
		schema.create_table_from_entity(age_restriction::Entity),
		schema.create_table_from_entity(user::Entity),
		schema.create_table_from_entity(user_preferences::Entity),
		schema.create_table_from_entity(library::Entity),
		schema.create_table_from_entity(library_config::Entity),
		schema.create_table_from_entity(reading_session::Entity),
		schema.create_table_from_entity(reading_device::Entity),
		schema.create_table_from_entity(tag::Entity),
		schema.create_table_from_entity(media_tag::Entity),
		schema.create_table_from_entity(server_config::Entity),
		schema.create_table_from_entity(refresh_token::Entity),
		schema.create_table_from_entity(session::Entity),
		// NoirPanther tables: fork queries (content rules, multi-folder libraries,
		// series merging) reference them from the shared `*_for_user` selects
		schema.create_table_from_entity(content_access_rule::Entity),
		schema.create_table_from_entity(library_path::Entity),
		schema.create_table_from_entity(series_merge::Entity),
		// failed sign-ins are counted per address from this table (login throttling)
		schema.create_table_from_entity(user_login_activity::Entity),
		// book clubs: the fork pages their rosters by cursor
		schema.create_table_from_entity(book_club::Entity),
		schema.create_table_from_entity(book_club_member::Entity),
		schema.create_table_from_entity(book_club_book::Entity),
		schema.create_table_from_entity(book_club_discussion::Entity),
		// offline reading (E3) wraps a book to a device's registered public key
		schema.create_table_from_entity(device_public_key::Entity),
	];

	for stmt in tables {
		db.execute(db.get_database_backend().build(&stmt)).await?;
	}

	// Indexes the entities do not carry but the code relies on. Building a schema from entities
	// misses them, and the failure is a runtime 500 ("ON CONFLICT clause does not match any
	// PRIMARY KEY or UNIQUE constraint"), not a missing table.
	db.execute_unprepared(
		"CREATE UNIQUE INDEX IF NOT EXISTS device_public_keys_user_id_device_id_key \
		 ON device_public_keys (user_id, device_id)",
	)
	.await?;

	Ok(())
}
