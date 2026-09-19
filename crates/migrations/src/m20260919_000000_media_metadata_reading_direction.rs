use sea_orm_migration::prelude::*;

/// NoirPanther: a per-book reading direction taken from the file's own metadata
/// (ComicInfo.xml `Manga` = `YesAndRightToLeft`). NULL = not specified by the file, the
/// library's `default_reading_dir` applies.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		if manager
			.has_column("media_metadata", "reading_direction")
			.await?
		{
			return Ok(());
		}

		manager
			.alter_table(
				Table::alter()
					.table(MediaMetadata::Table)
					.add_column(
						ColumnDef::new(MediaMetadata::ReadingDirection)
							.text()
							.null(),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(MediaMetadata::Table)
					.drop_column(MediaMetadata::ReadingDirection)
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum MediaMetadata {
	Table,
	ReadingDirection,
}
