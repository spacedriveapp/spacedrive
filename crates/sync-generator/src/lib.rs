#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use prisma_client_rust_sdk::{
	prelude::*,
	prisma::prisma_models::walkers::{
		FieldWalker, ModelWalker, RefinedFieldWalker, RelationFieldWalker,
	},
};

mod attribute;
mod model;
mod sync_data;

use attribute::{model_attributes, Attribute, AttributeFieldValue};

#[derive(Debug, serde::Serialize, thiserror::Error)]
enum Error {}

#[derive(serde::Deserialize)]
struct SDSyncGenerator {}

#[derive(Clone)]
pub enum ModelSyncType<'a> {
	Local {
		id: FieldWalker<'a>,
	},
	// Owned {
	// 	id: FieldVec<'a>,
	// },
	Shared {
		id: FieldWalker<'a>,
		// model ids help reduce storage cost of sync messages
		model_id: u16,
	},
	Relation {
		group: RelationFieldWalker<'a>,
		item: RelationFieldWalker<'a>,
		model_id: u16,
	},
}

impl<'a> ModelSyncType<'a> {
	fn from_attribute(attr: &Attribute<'_>, model: ModelWalker<'a>) -> Option<Self> {
		Some(match attr.name {
			"local" | "shared" => {
				let id = attr
					.field("id")
					.and_then(|field| match field {
						AttributeFieldValue::Single(s) => Some(s),
						AttributeFieldValue::List(_) => None,
					})
					.and_then(|name| model.fields().find(|f| f.name() == *name))?;

				match attr.name {
					"local" => Self::Local { id },
					"shared" => Self::Shared {
						id,
						model_id: attr
							.field("modelId")
							.and_then(|a| a.as_single())
							.and_then(|s| s.parse().ok())?,
					},
					_ => return None,
				}
			}
			"relation" => {
				let get_field = |name| {
					attr.field(name)
						.and_then(|field| match field {
							AttributeFieldValue::Single(s) => Some(*s),
							AttributeFieldValue::List(_) => None,
						})
						.and_then(|name| {
							if let RefinedFieldWalker::Relation(r) = model
								.fields()
								.find(|f| f.name() == name)
								.unwrap_or_else(|| panic!("'{name}' field not found"))
								.refine()
							{
								Some(r)
							} else {
								None
							}
						})
						.unwrap_or_else(|| panic!("'{name}' must be a relation field"))
				};

				Self::Relation {
					item: get_field("item"),
					group: get_field("group"),
					model_id: attr
						.field("modelId")
						.and_then(|a| a.as_single())
						.and_then(|s| s.parse().ok())?,
				}
			}
			// "owned" => Self::Owned { id },
			_ => return None,
		})
	}

	fn sync_id(&self) -> Vec<FieldWalker<'_>> {
		match self {
			// Self::Owned { id } => id.clone(),
			Self::Local { id, .. } | Self::Shared { id, .. } => vec![*id],
			Self::Relation { group, item, .. } => vec![(*group).into(), (*item).into()],
		}
	}
}

impl ToTokens for ModelSyncType<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			Self::Local { .. } => "Local",
			// Self::Owned { .. } => "Owned",
			Self::Shared { .. } => "Shared",
			Self::Relation { .. } => "Relation",
		};

		tokens.append(format_ident!("{variant}SyncType"));
	}
}

pub type ModelWithSyncType<'a> = (ModelWalker<'a>, Option<ModelSyncType<'a>>);

impl PrismaGenerator for SDSyncGenerator {
	const NAME: &'static str = "SD Sync Generator";
	const DEFAULT_OUTPUT: &'static str = "prisma-sync.rs";

	type Error = Error;

	fn generate(self, args: GenerateArgs<'_>) -> Result<Module, Self::Error> {
		let db = &args.schema.db;

		let models_with_sync_types = db
			.walk_models()
			.map(|model| (model, model_attributes(model)))
			.map(|(model, attributes)| {
				let sync_type = attributes
					.into_iter()
					.find_map(|a| ModelSyncType::from_attribute(&a, model));

				(model, sync_type)
			})
			.collect::<Vec<_>>();

		let model_sync_data = sync_data::enumerate(&models_with_sync_types);

		let mut module = Module::new(
			"root",
			quote! {
				use crate::prisma;

				#model_sync_data
			},
		);
		models_with_sync_types
			.into_iter()
			.map(model::module)
			.for_each(|model| module.add_submodule(model));

		Ok(module)
	}
}

pub fn run() {
	SDSyncGenerator::run();
}
