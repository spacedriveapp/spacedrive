use anyhow::Result;
use cargo_metadata::CargoOpt;
use clap::Parser;
use std::{fs::File, path::PathBuf};
use types::{
	backend::BackendDependency,
	cli::{Action, Arguments},
	frontend::FrontendDependency,
};

pub mod types;

const FOSSA_BASE_URL: &str =
	"https://app.fossa.com/api/revisions/git%2Bgithub.com%2Fspacedriveapp%2Fspacedrive%24";

#[tokio::main]
async fn main() -> Result<()> {
	let args = Arguments::parse();

	match args.action {
		Action::Frontend(sub_args) => write_frontend_deps(sub_args.revision, sub_args.path).await,
		Action::Backend(sub_args) => {
			write_backend_deps(sub_args.manifest_path, sub_args.output_path)
		}
	}
}

fn write_backend_deps(manifest_path: PathBuf, output_path: PathBuf) -> Result<()> {
	let cmd = cargo_metadata::MetadataCommand::new()
		.manifest_path(manifest_path)
		.features(CargoOpt::AllFeatures)
		.exec()?;

	let deps: Vec<BackendDependency> = cmd
		.packages
		.into_iter()
		.filter_map(|p| {
			if !cmd.workspace_members.iter().any(|t| &p.id == t) {
				let dep = BackendDependency {
					title: p.name,
					description: p.description,
					url: p.repository,
					version: p.version.to_string(),
					authors: p.authors,
					license: p.license,
				};

				Some(dep)
			} else {
				None
			}
		})
		.collect();

	let mut file = File::create(output_path)?;
	serde_json::to_writer(&mut file, &deps)?;

	Ok(())
}

async fn write_frontend_deps(rev: String, path: PathBuf) -> Result<()> {
	let url = format!("{FOSSA_BASE_URL}{rev}/dependencies");

	let response = reqwest::get(url).await?.text().await?;
	let json: Vec<types::frontend::Dependency> = serde_json::from_str(&response)?;

	let deps: Vec<FrontendDependency> = json
		.into_iter()
		.map(|dep| FrontendDependency {
			title: dep.project.title,
			authors: dep.project.authors,
			description: dep.project.description,
			url: dep.project.url,
			license: dep.licenses,
		})
		.collect();

	let mut file = File::create(path)?;
	serde_json::to_writer(&mut file, &deps)?;

	Ok(())
}
