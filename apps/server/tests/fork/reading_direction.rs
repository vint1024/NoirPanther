use models::entity::media_metadata;
use sea_orm::{ActiveValue, EntityTrait};
use serde_json::json;
use tests::fake_data;

use crate::common::{series::setup_single_series_with_n_books, TestApp};

/// A29: a comic opens the way its file says it should. `ComicInfo.xml`'s `Manga` tag becomes a
/// per-book reading direction (upstream only has a per-library default), and both the web reader
/// and the mobile app start such books right-to-left.
#[tokio::test]
async fn a_book_reports_the_reading_direction_from_its_file() {
	let app = TestApp::new_with_default_user().await;
	let library = fake_data::Library {
		id: Some("direction_lib".to_string()),
		name: Some("Direction library".to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	let (_, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("direction_series".to_string()),
			name: Some("Direction series".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		2,
	)
	.await;
	let manga = books.first().expect("two books");
	let plain = books.get(1).expect("two books");

	media_metadata::Entity::insert(media_metadata::ActiveModel {
		media_id: ActiveValue::Set(Some(manga.id.clone())),
		reading_direction: ActiveValue::Set(Some(
			models::shared::enums::ReadingDirection::Rtl,
		)),
		..Default::default()
	})
	.exec(app.conn())
	.await
	.expect("metadata insert");

	let direction_of = |id: String| {
		let app = &app;
		async move {
			let response = app
				.execute_gql(
					r#"query Book($id: ID!) { mediaById(id: $id) { readingDirection } }"#,
					Some(json!({ "id": id })),
				)
				.await;
			response["data"]["mediaById"]["readingDirection"]
				.as_str()
				.map(str::to_string)
		}
	};

	assert_eq!(
		direction_of(manga.id.clone()).await.as_deref(),
		Some("RTL"),
		"a manga file must open right-to-left"
	);
	assert_eq!(
		direction_of(plain.id.clone()).await.as_deref(),
		Some("LTR"),
		"a book with nothing in its file falls back to the library default"
	);
}
