use async_graphql::{Context, Object, Result, ID};
use models::{
	entity::{book_club_member, user::AuthUser},
	shared::book_club::BookClubMemberRole,
};
use sea_orm::prelude::*;

use crate::{
	data::{AuthContext, CoreContext},
	guard::BookClubRoleGuard,
	input::book_club::CreateBookClubMemberInput,
	object::book_club_member::BookClubMember,
};

#[derive(Default)]
pub struct BookClubMemberMutation;

#[Object]
impl BookClubMemberMutation {
	/// Creates a new member in the book club
	#[graphql(
		guard = "BookClubRoleGuard::new(book_club_id.as_ref(), BookClubMemberRole::Admin)"
	)]
	async fn create_book_club_member(
		&self,
		ctx: &Context<'_>,
		book_club_id: ID,
		input: CreateBookClubMemberInput,
	) -> Result<BookClubMember> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		refuse_adding_someone_else(user, &input)?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let created_member = input
			.into_active_model(book_club_id.as_ref())
			.insert(conn)
			.await?;

		Ok(BookClubMember::from(created_member))
	}

	/// Removes a member from the book club
	#[graphql(
		guard = "BookClubRoleGuard::new(book_club_id.as_ref(), BookClubMemberRole::Admin)"
	)]
	async fn remove_book_club_member(
		&self,
		ctx: &Context<'_>,
		// Note: This is a false positive since we use it for the guard
		#[allow(unused_variables)] book_club_id: ID,
		member_id: ID,
	) -> Result<BookClubMember> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let member = book_club_member::Entity::find_by_id(member_id.as_ref())
			.one(conn)
			.await?
			.ok_or("Member not found")?;

		if member.role == BookClubMemberRole::Creator {
			return Err("Cannot remove the creator of the book club".into());
		}

		member.clone().delete(conn).await?;

		Ok(BookClubMember::from(member))
	}

	/// Deletes the membership of the caller to the target book club
	async fn leave_book_club(
		&self,
		ctx: &Context<'_>,
		book_club_id: ID,
	) -> Result<BookClubMember> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let member =
			book_club_member::Entity::find_by_club_for_user(user, book_club_id.as_ref())
				.one(conn)
				.await?
				.ok_or("You are not a member of this club or it does not exist")?;

		member.clone().delete(conn).await?;

		Ok(BookClubMember::from(member))
	}
}

/// NoirPanther: nobody is put in a book club without saying yes. Upstream lets a club admin add
/// anyone outright (it only never worked: `joined_at` was left empty); here people join by
/// accepting an invitation (`createBookClubInvitation` → `respondToBookClubInvitation`), which
/// they can decline. Pinned by `tests/fork/book_clubs.rs` so an upstream change can't quietly
/// bring direct adds back.
fn refuse_adding_someone_else(
	user: &AuthUser,
	input: &CreateBookClubMemberInput,
) -> Result<()> {
	if input.user_id != user.id {
		return Err(
			"People join a book club by accepting an invitation: invite them instead"
				.into(),
		);
	}
	Ok(())
}
