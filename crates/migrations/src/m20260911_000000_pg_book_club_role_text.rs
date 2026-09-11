use sea_orm::{DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `BookClubMemberRole` is a string-backed enum (`MEMBER`, `MODERATOR`, `ADMIN`,
/// `CREATOR`) but the schema declares `book_club_members.role` and
/// `book_club_invitations.role` as INTEGER. SQLite's type affinity silently
/// stores the strings anyway; PostgreSQL rejects them, which breaks book clubs
/// there. Make the columns TEXT on PostgreSQL (nothing to do on SQLite, where
/// they already hold text). Runs after the fork's other migrations so it also
/// fixes databases that were created on PostgreSQL before this fix.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let conn = manager.get_connection();
		if conn.get_database_backend() == DbBackend::Postgres {
			for table in ["book_club_members", "book_club_invitations"] {
				conn.execute(Statement::from_string(
					DbBackend::Postgres,
					format!(
						r#"ALTER TABLE {table}
						ALTER COLUMN role TYPE TEXT
						USING (CASE role::text
							WHEN '0' THEN 'MEMBER'
							WHEN '1' THEN 'MODERATOR'
							WHEN '2' THEN 'ADMIN'
							WHEN '3' THEN 'CREATOR'
							ELSE role::text
						END)"#
					),
				))
				.await?;
			}
		}
		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let conn = manager.get_connection();
		if conn.get_database_backend() == DbBackend::Postgres {
			for table in ["book_club_members", "book_club_invitations"] {
				conn.execute(Statement::from_string(
					DbBackend::Postgres,
					format!(
						r#"ALTER TABLE {table}
						ALTER COLUMN role TYPE INTEGER
						USING (CASE role
							WHEN 'MEMBER' THEN 0
							WHEN 'MODERATOR' THEN 1
							WHEN 'ADMIN' THEN 2
							WHEN 'CREATOR' THEN 3
							ELSE 0
						END)"#
					),
				))
				.await?;
			}
		}
		Ok(())
	}
}
