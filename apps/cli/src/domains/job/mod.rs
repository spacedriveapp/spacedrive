mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::{
    jobs::{
        info::output::JobInfoOutput,
        list::output::JobListOutput,
    },
    libraries::list::query::ListLibrariesQuery,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum JobCmd {
    /// List jobs
    List(JobListArgs),
    /// Job info
    Info(JobInfoArgs),
}

pub async fn run(ctx: &Context, cmd: JobCmd) -> Result<()> {
    match cmd {
        JobCmd::List(args) => {
            let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_query!(ctx, ListLibrariesQuery::basic());
            if libs.is_empty() {
                println!("No libraries found");
                return Ok(());
            }

            for lib in libs {
                let out: JobListOutput = execute_query!(ctx, args.to_query(lib.id));
                print_output!(ctx, &out, |o: &JobListOutput| {
                    for j in &o.jobs {
                        println!(
                            "- {} {} {} {:?}",
                            j.id,
                            j.name,
                            (j.progress * 100.0) as u32,
                            j.status
                        );
                    }
                });
            }
        }
        JobCmd::Info(args) => {
            let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_query!(ctx, ListLibrariesQuery::basic());
            let _lib = libs.get(0).ok_or_else(|| anyhow::anyhow!("No libraries found"))?;

            let out: Option<JobInfoOutput> = execute_query!(ctx, args.to_query());
            print_output!(ctx, &out, |o: &Option<JobInfoOutput>| {
                match o {
                    Some(j) => println!(
                        "{} {} {}% {:?}",
                        j.id,
                        j.name,
                        (j.progress * 100.0) as u32,
                        j.status
                    ),
                    None => println!("Job not found"),
                }
            });
        }
    }
    Ok(())
}
