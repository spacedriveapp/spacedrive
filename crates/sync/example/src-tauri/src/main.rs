#![cfg_attr(
	all(not(debug_assertions), target_os = "windows"),
	windows_subsystem = "windows"
)]

use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

use rspc::*;
use sd_sync::*;

#[derive(Default)]
struct Ctx {
	pub dbs: HashMap<Uuid, Db>,
}

type Router = rspc::Router<Arc<Mutex<Ctx>>>;

#[tokio::main]
async fn main() {
	let router = Arc::new(
		<Router>::new()
			.config(Config::new().export_ts_bindings(
				PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts"),
			))
			.mutation("createDatabase", |r| {
				r(|ctx, _: ()| async move {
					let dbs = &mut ctx.lock().await.dbs;
					let uuid = Uuid::new_v4();

					dbs.insert(uuid.clone(), Db::new(uuid.clone()));

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

					Ok(dbs.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>())
				})
			})
			.query("db.tags", |r| {
				r(|ctx, id: String| async move {
					let dbs = &mut ctx.lock().await.dbs;

					let id = id.parse().unwrap();

					println!("{:?}", &dbs);

					Ok(dbs.get(&id).unwrap().tags.clone())
				})
			})
			.build(),
	);

	let ctx = Arc::new(Mutex::new(Default::default()));

	tauri::Builder::default()
		.plugin(rspc::integrations::tauri::plugin(router, move || {
			ctx.clone()
		}))
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
