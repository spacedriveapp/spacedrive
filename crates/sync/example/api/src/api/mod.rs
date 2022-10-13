use std::collections::HashMap;
use std::sync::{
	atomic::{AtomicI32, Ordering},
	Arc,
};

// use crate::prisma;
use rspc::*;
use sd_sync::*;
use serde_json::*;
use std::path::PathBuf;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct Ctx {
	pub dbs: HashMap<Uuid, Db>,
}

type Router = rspc::Router<Arc<Mutex<Ctx>>>;

static ID: AtomicI32 = AtomicI32::new(0);

fn to_map(v: &impl serde::Serialize) -> serde_json::Map<String, Value> {
	match to_value(&v).unwrap() {
		Value::Object(m) => m,
		_ => unreachable!(),
	}
}

pub(crate) fn new() -> RouterBuilder<Arc<Mutex<Ctx>>> {
	Router::new()
		.config(Config::new().export_ts_bindings(
			PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/src/utils/bindings.ts"),
		))
		.mutation("createDatabase", |r| {
			r(|ctx, _: ()| async move {
				let dbs = &mut ctx.lock().await.dbs;
				let uuid = Uuid::new_v4();

				dbs.insert(uuid, Db::new(uuid));

				println!("{:?}", dbs);

				Ok(uuid)
			})
		})
		.mutation("removeDatabases", |r| {
			r(|ctx, _: ()| async move {
				let dbs = &mut ctx.lock().await.dbs;

				dbs.drain();

				Ok(())
			})
		})
		.query("dbs", |r| {
			r(|ctx, _: ()| async move {
				let dbs = &mut ctx.lock().await.dbs;

				Ok(dbs.iter().map(|(id, _)| *id).collect::<Vec<_>>())
			})
		})
		.query("db.tags", |r| {
			r(|ctx, id: String| async move {
				let dbs = &mut ctx.lock().await.dbs;

				let id = id.parse().unwrap();

				Ok(dbs.get(&id).unwrap().tags.clone())
			})
		})
		.query("file_path.list", |r| {
			r(|ctx, id: String| async move {
				let dbs = &mut ctx.lock().await.dbs;

				let db = dbs.get(&id.parse().unwrap()).unwrap();

				let file_paths = db.file_paths.values().map(Clone::clone).collect::<Vec<_>>();

				Ok(file_paths)
			})
		})
		.mutation("file_path.create", |r| {
			r(|ctx, db: String| async move {
				let dbs = &mut ctx.lock().await.dbs;

				let db = dbs.get_mut(&db.parse().unwrap()).unwrap();

				let id = ID.fetch_add(1, Ordering::SeqCst);

				let file_path = FilePath {
					id,
					path: String::new(),
					file: None,
				};

				db.file_paths.insert(id, file_path.clone());

				let message = db.create_crdt_operation(CRDTOperationType::Owned(OwnedOperation {
					model: "FilePath".to_string(),
					items: vec![OwnedOperationItem {
						id: serde_json::to_value(id).unwrap(),
						data: OwnedOperationData::Create(to_map(&file_path)),
					}],
				}));

				file_path
			})
		})
}
