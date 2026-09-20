use models::entity::{
	age_restriction, api_key, book_club, book_club_book, book_club_discussion,
	book_club_member, content_access_rule, kobo_sync_session, library, library_config,
	library_exclusion, library_path, media, media_analysis, media_metadata, media_tag,
	reading_device, reading_session, refresh_token, series, series_merge,
	series_metadata, server_config, session, tag, user, user_login_activity,
	user_preferences,
};
use sea_orm::{ConnectionTrait, DbBackend, DbConn, DbErr, Schema};
pub async fn test_database() -> DbConn {
	// Connect through models::db so custom SQL functions (ulower) exist in
	// test databases too — content-rule queries reference them
	let db = models::db::connect_sqlite("sqlite::memory:")
		.await
		.expect("failed to connect to test database");

	create_database_tables(&db)
		.await
		.expect("failed to create test database tables");

	db
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
	];

	for stmt in tables {
		db.execute(db.get_database_backend().build(&stmt)).await?;
	}

	Ok(())
}
