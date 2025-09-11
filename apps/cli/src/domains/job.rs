use anyhow::Result;
use clap::Subcommand;
use uuid::Uuid;

use crate::context::{Context, OutputFormat};
use crate::util::output::print_json;

#[derive(Subcommand, Debug)]
pub enum JobCmd {
	/// List jobs
	List { #[arg(long)] status: Option<String> },
	/// Job info
	Info { job_id: Uuid },
}

pub async fn run(ctx: &Context, cmd: JobCmd) -> Result<()> {
	match cmd {
		JobCmd::List { status } => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
				.core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			if libs.is_empty() { println!("No libraries found"); }
			for lib in libs {
				let status_parsed = status.as_deref().and_then(|s| s.parse::<sd_core::infra::job::types::JobStatus>().ok());
				let out: sd_core::ops::jobs::list::output::JobListOutput = ctx.core.query(&sd_core::ops::jobs::list::query::JobListQuery { library_id: lib.id, status: status_parsed }).await?;
				match ctx.format {
					OutputFormat::Human => {
						for j in out.jobs { println!("- {} {} {} {:?}", j.id, j.name, (j.progress * 100.0) as u32, j.status); }
					}
					OutputFormat::Json => print_json(&out),
				}
			}
		}
		JobCmd::Info { job_id } => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
				.core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			let lib = libs.get(0).ok_or_else(|| anyhow::anyhow!("No libraries found"))?;
			let out: Option<sd_core::ops::jobs::info::output::JobInfoOutput> = ctx.core.query(&sd_core::ops::jobs::info::query::JobInfoQuery { library_id: lib.id, job_id }).await?;
			match (ctx.format, out) {
				(OutputFormat::Human, Some(j)) => println!("{} {} {}% {:?}", j.id, j.name, (j.progress * 100.0) as u32, j.status),
				(OutputFormat::Json, Some(j)) => print_json(&j),
				(_, None) => println!("Job not found"),
			}
		}
	}
	Ok(())
}

