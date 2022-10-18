use normi::{typed, Object};
use rspc::Type;
use serde::Serialize;

use super::RouterBuilder;

#[derive(Serialize, Type, Object)]
#[normi(rename = "org")]
pub struct Organisation {
	#[normi(id)]
	pub id: String,
	pub name: String,
	#[normi(refr)]
	pub users: Vec<User>,
	#[normi(refr)]
	pub owner: User,
	pub non_normalised_data: Vec<()>,
}

#[derive(Serialize, Type, Object)]
pub struct User {
	#[normi(id)]
	pub id: String,
	pub name: String,
}

#[derive(Serialize, Type, Object)]
pub struct CompositeId {
	#[normi(id)]
	pub org_id: String,
	#[normi(id)]
	pub user_id: String,
}

pub fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.query("version", |t| t(|_, _: ()| "0.1.0"))
		.query("userSync", |t| {
			t.resolver(|_, _: ()| User {
				id: "1".to_string(),
				name: "Monty Beaumont".to_string(),
			})
			.map(typed)
		})
		.query("user", |t| {
			t.resolver(|_, _: ()| async move {
				Ok(User {
					id: "1".to_string(),
					name: "Monty Beaumont".to_string(),
				})
			})
			.map(typed)
		})
		.query("org", |t| {
			t.resolver(|_, _: ()| async move {
				Ok(Organisation {
					id: "org-1".into(),
					name: "Org 1".into(),
					users: vec![
						User {
							id: "user-1".into(),
							name: "Monty Beaumont".into(),
						},
						User {
							id: "user-2".into(),
							name: "Millie Beaumont".into(),
						},
						User {
							id: "user-3".into(),
							name: "Oscar Beaumont".into(),
						},
					],
					owner: User {
						id: "user-1".into(),
						name: "Monty Beaumont".into(),
					},
					non_normalised_data: vec![(), ()],
				})
			})
			.map(typed)
		})
		.query("composite", |t| {
			t.resolver(|_, _: ()| async move {
				Ok(CompositeId {
					org_id: "org-1".into(),
					user_id: "user-1".into(),
				})
			})
			.map(typed)
		})
}
