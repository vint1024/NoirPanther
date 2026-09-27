use sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// NoirPanther: `media.extension` came straight from the file name, so `Book.PDF` was stored as
/// `PDF` while the code compares it verbatim — `book.extension == "epub"` in the media, series
/// and library mutations, `cbz`/`zip` in the app's offline pager. The scanner now stores it
/// lowercase (`core/src/media/prepare.rs`); this folds the rows stored before. Nothing builds a
/// path from the column (the file's real path is `media.path`), so the change is safe to keep.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let conn = manager.get_connection();
		conn.execute(Statement::from_string(
			conn.get_database_backend(),
			"UPDATE media SET extension = LOWER(extension) WHERE extension <> LOWER(extension)",
		))
		.await?;
		Ok(())
	}

	async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
		// The original spelling isn't kept anywhere; lowercase is valid for every reader.
		Ok(())
	}
}
