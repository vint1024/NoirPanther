use sea_orm::{DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `device_public_keys.created_at` was created as `TIMESTAMP` (no time zone) while the entity
/// reads it as `DateTimeWithTimeZone`. SQLite does not care; PostgreSQL refuses to decode the
/// column, so **every** offline download (`POST /media/{id}/offline`, the E3 flow) answered 500
/// on a PostgreSQL server — from the migration to PostgreSQL until this fix. It is the only
/// timestamp column in the schema that was not `timestamptz`.
///
/// Found by running the integration suite against PostgreSQL
/// (`TEST_DATABASE_URL=…`), which is exactly the hole that run was added to close.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let conn = manager.get_connection();
		if conn.get_database_backend() == DbBackend::Postgres {
			// Existing rows were written as UTC by the server, so read them back as UTC.
			conn.execute(Statement::from_string(
				DbBackend::Postgres,
				r#"ALTER TABLE device_public_keys
				ALTER COLUMN created_at TYPE TIMESTAMPTZ
				USING created_at AT TIME ZONE 'UTC'"#,
			))
			.await?;
		}
		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let conn = manager.get_connection();
		if conn.get_database_backend() == DbBackend::Postgres {
			conn.execute(Statement::from_string(
				DbBackend::Postgres,
				r#"ALTER TABLE device_public_keys
				ALTER COLUMN created_at TYPE TIMESTAMP
				USING created_at AT TIME ZONE 'UTC'"#,
			))
			.await?;
		}
		Ok(())
	}
}
