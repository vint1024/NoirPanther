use async_graphql::{InputObject, InputType, OneofObject};
use filter_gen::IntoFilter;
use models::{
	db::UNICODE_LOWER_FN,
	shared::enums::{FileStatus, LibraryType, ReadingStatus},
};
use sea_orm::{
	prelude::DateTimeWithTimeZone,
	sea_query::{Alias, ConditionExpression, Expr, Func, FunctionCall},
	ColumnTrait, Condition, Value,
};
use serde::{Deserialize, Serialize};

pub mod keyword;
pub mod library;
pub mod log;
pub mod media;
pub mod media_metadata;
pub mod series;
pub mod series_metadata;

use keyword::{like_contains, like_ends_with, like_starts_with};

// TODO: This probably needs a rewrite to make it more compatible with async-graphql. The big issue is generics
// with input objects. Look at and yoink from seaography for how they are doing things

// Note: See https://github.com/serde-rs/json/issues/501

// NOTE: I originally went for IntoCondition, but that is a trait for sea-query and
// I wanted to avoid conflicts in the naming
pub trait IntoFilter {
	fn into_filter(self) -> sea_orm::Condition;
}

#[derive(OneofObject, Clone, Debug, Serialize, Deserialize)]
#[graphql(concrete(name = "FieldFilterString", params(String)))]
#[graphql(concrete(name = "FieldFilterFileStatus", params(FileStatus)))]
#[serde(rename_all = "camelCase")]
pub enum StringLikeFilter<T>
where
	T: InputType,
{
	Eq(T),
	Neq(T),
	AnyOf(Vec<T>),
	NoneOf(Vec<T>),
	Like(T),
	LikeAnyOf(Vec<T>),
	LikeNoneOf(Vec<T>),
	Contains(T),
	Excludes(T),
	StartsWith(T),
	EndsWith(T),
}

/// Wrap a column in the custom Unicode-lowercasing SQL function so a LIKE
/// against it folds case across the full Unicode range. SQLite's own
/// `lower()`/LIKE only fold ASCII, so without this a search for "рикман" would
/// never match a row stored as "Рикман". `ulower` is registered by
/// `models::db::connect_sqlite`; see [`models::db`].
fn ulower<C>(column: C) -> FunctionCall
where
	C: ColumnTrait,
{
	Func::cust(Alias::new(UNICODE_LOWER_FN)).arg(Expr::col(column.as_column_ref()))
}

pub(crate) fn apply_string_filter<C, T>(
	column: C,
	filter: StringLikeFilter<T>,
) -> Condition
where
	C: ColumnTrait + Copy,
	T: InputType + Into<Value> + Into<String>,
{
	match filter {
		StringLikeFilter::Eq(value) => Condition::all().add(column.eq(value)),
		StringLikeFilter::Neq(value) => Condition::all().add(column.ne(value)),
		StringLikeFilter::AnyOf(values) => Condition::all().add(column.is_in(values)),
		StringLikeFilter::NoneOf(values) => {
			Condition::all().add(column.is_not_in(values))
		},
		// The LIKE-family below all fold case across the full Unicode range via
		// ulower(). SQLite's native lower()/LIKE only fold ASCII, which left
		// Cyrillic/accented searches (e.g. "рикман", "шитенбург") effectively
		// case-sensitive. Both sides are lowercased: the column by ulower() in
		// SQL, the value by to_lowercase() in Rust (see `keyword`). On Postgres
		// `ulower` is registered as a thin wrapper over the (already Unicode-
		// aware) built-in lower(), so the generated SQL is identical on both
		// backends. Semantics follow upstream: `like`/`contains`/`excludes` are
		// substring matches, `startsWith`/`endsWith` are anchored; every literal
		// has its LIKE metacharacters escaped except `like`, which is a raw pattern.
		StringLikeFilter::Like(value) => {
			let v: String = value.into();
			Condition::all()
				.add(Expr::expr(ulower(column)).like(format!("%{}%", v.to_lowercase())))
		},
		StringLikeFilter::Contains(value) => {
			let v: String = value.into();
			Condition::all().add(Expr::expr(ulower(column)).like(like_contains(&v)))
		},
		StringLikeFilter::Excludes(value) => {
			let v: String = value.into();
			Condition::all().add(Expr::expr(ulower(column)).not_like(like_contains(&v)))
		},
		StringLikeFilter::StartsWith(value) => {
			let v: String = value.into();
			Condition::all().add(Expr::expr(ulower(column)).like(like_starts_with(&v)))
		},
		StringLikeFilter::EndsWith(value) => {
			let v: String = value.into();
			Condition::all().add(Expr::expr(ulower(column)).like(like_ends_with(&v)))
		},
		StringLikeFilter::LikeAnyOf(values) => {
			values.into_iter().fold(Condition::any(), |acc, value| {
				let v: String = value.into();
				acc.add(Expr::expr(ulower(column)).like(like_contains(&v)))
			})
		},
		StringLikeFilter::LikeNoneOf(values) => values
			.into_iter()
			.fold(Condition::any(), |acc, value| {
				let v: String = value.into();
				acc.add(Expr::expr(ulower(column)).like(like_contains(&v)))
			})
			.not(),
	}
}

#[cfg(test)]
mod string_filter_tests {
	use super::*;
	use models::entity::media;
	use sea_orm::{
		sea_query::SqliteQueryBuilder, EntityTrait, QueryFilter, QuerySelect, QueryTrait,
	};

	fn render(filter: StringLikeFilter<String>) -> String {
		media::Entity::find()
			.filter(apply_string_filter(media::Column::Name, filter))
			.select_only()
			.into_query()
			.to_string(SqliteQueryBuilder)
	}

	#[test]
	fn contains_folds_case_via_ulower() {
		let sql = render(StringLikeFilter::Contains("Рикман".to_string()));
		// Column side is ulower()'d and the value is lowercased + LIKE-wrapped.
		assert!(
			sql.contains(r#"ulower("media"."name") LIKE '%рикман%' ESCAPE '\'"#),
			"{sql}"
		);
	}

	#[test]
	fn contains_escapes_like_metacharacters() {
		let sql = render(StringLikeFilter::Contains("100%_x".to_string()));
		assert!(sql.contains(r#"LIKE '%100\%\_x%' ESCAPE '\'"#), "{sql}");
	}

	#[test]
	fn starts_and_ends_with_anchor_pattern() {
		let starts = render(StringLikeFilter::StartsWith("Алан".to_string()));
		assert!(
			starts.contains(r#"ulower("media"."name") LIKE 'алан%' ESCAPE '\'"#),
			"{starts}"
		);
		let ends = render(StringLikeFilter::EndsWith("Лилия".to_string()));
		assert!(
			ends.contains(r#"ulower("media"."name") LIKE '%лилия' ESCAPE '\'"#),
			"{ends}"
		);
	}

	#[test]
	fn eq_is_left_exact() {
		let sql = render(StringLikeFilter::Eq("Exact".to_string()));
		assert!(sql.contains(r#""media"."name" = 'Exact'"#), "{sql}");
		assert!(!sql.contains("ulower"), "{sql}");
	}
}

#[derive(InputObject, Clone, Debug, Serialize, Deserialize)]
#[graphql(concrete(name = "NumericRangeF32", params(f32)))]
#[graphql(concrete(name = "NumericRangeI32", params(i32)))]
#[graphql(concrete(name = "NumericRangeI64", params(i64)))]
#[graphql(concrete(name = "NumericRangeU32", params(u32)))]
#[graphql(concrete(name = "NumericRangeU64", params(u64)))]
#[graphql(concrete(name = "NumericRangeDateTime", params(DateTimeWithTimeZone)))]
#[serde(rename_all = "camelCase")]
pub struct NumericRange<T>
where
	T: InputType,
{
	pub from: T,
	pub to: T,
	pub inclusive: bool,
}

#[derive(OneofObject, Clone, Debug, Serialize, Deserialize)]
#[graphql(concrete(name = "NumericFilterF32", params(f32)))]
#[graphql(concrete(name = "NumericFilterI32", params(i32)))]
#[graphql(concrete(name = "NumericFilterI64", params(i64)))]
#[graphql(concrete(name = "NumericFilterU32", params(u32)))]
#[graphql(concrete(name = "NumericFilterU64", params(u64)))]
#[graphql(concrete(name = "NumericFilterDateTime", params(DateTimeWithTimeZone)))]
#[serde(rename_all = "camelCase")]
pub enum NumericFilter<T>
where
	T: InputType,
	NumericRange<T>: InputType,
{
	Eq(T),
	Neq(T),
	AnyOf(Vec<T>),
	NoneOf(Vec<T>),
	Gt(T),
	Gte(T),
	Lt(T),
	Lte(T),
	Range(NumericRange<T>),
}

pub(crate) fn apply_numeric_filter<C, T>(
	column: C,
	filter: NumericFilter<T>,
) -> impl Into<ConditionExpression>
where
	C: sea_orm::ColumnTrait,
	T: InputType + Into<Value>,
	NumericRange<T>: InputType,
{
	match filter {
		NumericFilter::Eq(value) => column.eq(value),
		NumericFilter::Neq(value) => column.ne(value),
		NumericFilter::AnyOf(values) => column.is_in(values),
		NumericFilter::NoneOf(values) => column.is_not_in(values),
		NumericFilter::Gt(value) => column.gt(value),
		NumericFilter::Gte(value) => column.gte(value),
		NumericFilter::Lt(value) => column.lt(value),
		NumericFilter::Lte(value) => column.lte(value),
		NumericFilter::Range(range) => {
			if range.inclusive {
				column.gte(range.from).and(column.lte(range.to))
			} else {
				column.gt(range.from).and(column.lt(range.to))
			}
		},
	}
}

#[derive(OneofObject, Clone, Debug, Serialize, Deserialize)]
#[graphql(concrete(name = "ComputedFilterReadingStatus", params(ReadingStatus)))]
#[graphql(concrete(name = "ComputedFilterLibraryType", params(LibraryType)))]
#[serde(rename_all = "camelCase")]
pub enum ConceptualFilter<T>
where
	T: InputType,
{
	Is(T),
	IsNot(T),
	IsAnyOf(Vec<T>),
	IsNoneOf(Vec<T>),
}

#[cfg(test)]
mod tests {
	use super::*;
	use models::entity::media;
	use pretty_assertions::assert_eq;
	use sea_orm::{prelude::*, sea_query::SqliteQueryBuilder, QuerySelect, QueryTrait};

	#[test]
	fn test_string_like_any_of() {
		let filter =
			StringLikeFilter::LikeAnyOf(vec!["test".to_string(), "example".to_string()]);
		let condition = apply_string_filter(media::Column::Name, filter);
		let query = media::Entity::find().filter(condition);
		let sql = query
			.select_only()
			.into_query()
			.to_string(SqliteQueryBuilder);

		// ulower()-wrapped + ESCAPE so the match folds case across the full
		// Unicode range (see apply_string_filter).
		assert_eq!(
			sql,
			r#"SELECT  FROM "media" WHERE ulower("media"."name") LIKE '%test%' ESCAPE '\' OR ulower("media"."name") LIKE '%example%' ESCAPE '\'"#
		);
	}

	#[test]
	fn test_string_like_none_of() {
		let filter =
			StringLikeFilter::LikeNoneOf(vec!["test".to_string(), "example".to_string()]);
		let condition = apply_string_filter(media::Column::Name, filter);
		let query = media::Entity::find().filter(condition);
		let sql = query
			.select_only()
			.into_query()
			.to_string(SqliteQueryBuilder);

		assert_eq!(
			sql,
			r#"SELECT  FROM "media" WHERE NOT (ulower("media"."name") LIKE '%test%' ESCAPE '\' OR ulower("media"."name") LIKE '%example%' ESCAPE '\')"#
		);
	}

	#[test]
	fn test_contains_escapes_wildcards_in_the_pattern() {
		let sql = media::Entity::find()
			.filter(apply_string_filter(
				media::Column::Name,
				StringLikeFilter::Contains("50%".to_string()),
			))
			.select_only()
			.into_query()
			.to_string(SqliteQueryBuilder);

		assert_eq!(
			sql,
			r#"SELECT  FROM "media" WHERE ulower("media"."name") LIKE '%50\%%' ESCAPE '\'"#
		);
	}

	#[test]
	fn test_starts_and_ends_with_escape_wildcards() {
		let starts = media::Entity::find()
			.filter(apply_string_filter(
				media::Column::Name,
				StringLikeFilter::StartsWith("file_".to_string()),
			))
			.select_only()
			.into_query()
			.to_string(SqliteQueryBuilder);
		assert_eq!(
			starts,
			r#"SELECT  FROM "media" WHERE ulower("media"."name") LIKE 'file\_%' ESCAPE '\'"#
		);

		let ends = media::Entity::find()
			.filter(apply_string_filter(
				media::Column::Name,
				StringLikeFilter::EndsWith("_v1".to_string()),
			))
			.select_only()
			.into_query()
			.to_string(SqliteQueryBuilder);
		assert_eq!(
			ends,
			r#"SELECT  FROM "media" WHERE ulower("media"."name") LIKE '%\_v1' ESCAPE '\'"#
		);
	}

	#[test]
	fn test_excludes_is_a_substring_negation() {
		let sql = media::Entity::find()
			.filter(apply_string_filter(
				media::Column::Name,
				StringLikeFilter::Excludes("annual".to_string()),
			))
			.select_only()
			.into_query()
			.to_string(SqliteQueryBuilder);

		assert_eq!(
			sql,
			r#"SELECT  FROM "media" WHERE ulower("media"."name") NOT LIKE '%annual%' ESCAPE '\'"#
		);
	}
}
