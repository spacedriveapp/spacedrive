use std::{
	collections::{hash_map::Entry, HashMap},
	path::PathBuf,
};

use crate::{
	invalidate_query,
	job::{job_without_data, JobManager, JobReport, JobStatus},
	location::{find_location, LocationError},
	object::{
		file_identifier::file_identifier_job::FileIdentifierJobInit,
		preview::thumbnailer_job::ThumbnailerJobInit,
		validation::validator_job::ObjectValidatorJobInit,
	},
	prisma::{job, location, SortOrder},
};

use chrono::{DateTime, Utc};
use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use specta::Type;

use uuid::Uuid;

use super::{utils::library, CoreEvent, Ctx, R};
use tokio::time::{interval, Duration};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("progress", {
			// Listen for updates from the job manager
			// - the client listens for events containing an updated JobReport
			// - the client replaces its local copy of the JobReport using the index provided by the reports procedure
			// - this should be used with the ephemeral sync engine
			R.with2(library())
				.subscription(|(ctx, _), job_uuid: Uuid| async move {
					let mut event_bus_rx = ctx.event_bus.0.subscribe();
					let mut tick = interval(Duration::from_secs_f64(1.0 / 30.0));

					async_stream::stream! {
						loop {
							let progress_event = loop {
								if let Ok(CoreEvent::JobProgress(progress_event)) = event_bus_rx.recv().await {
									if progress_event.id == job_uuid {
										break progress_event;
									}
								}
							};

							yield progress_event;

							loop {
								tokio::select! { biased;
									_ = tick.tick() => { break; },
									_ = event_bus_rx.recv() => {
										// event was killed by the void
									},
								}
							}
						}
					}
				})
		})
		.procedure("reports", {
			// Reports provides the client with a list of JobReports
			// - we query with a custom select! to avoid returning paused job cache `job.data`
			// - results must include running jobs, and be combined with the in-memory state
			//	  this is to ensure the client will always get the correct initial state
			// - jobs are sorted in to groups by their action
			// - TODO: refactor grouping system to a many-to-many table
			#[derive(Debug, Clone, Serialize, Deserialize, Type)]
			pub struct JobGroup {
				id: String,
				action: String,
				status: JobStatus,
				created_at: DateTime<Utc>,
				jobs: Vec<JobReport>,
			}
			#[derive(Debug, Clone, Serialize, Deserialize, Type)]
			pub struct JobGroups {
				groups: Vec<JobGroup>,
				index: HashMap<String, i32>, // maps job ids to their group index
			}
			R.with2(library())
				.query(|(ctx, library), _: ()| async move {
					let mut groups: HashMap<String, JobGroup> = HashMap::new();

					let job_reports: Vec<JobReport> = library
						.db
						.job()
						.find_many(vec![])
						.order_by(job::date_created::order(SortOrder::Desc))
						.take(100)
						.select(job_without_data::select())
						.exec()
						.await?
						.into_iter()
						.flat_map(JobReport::try_from)
						.collect();

					let active_reports = ctx.jobs.get_active_reports().await;

					for job in job_reports {
						// action name and group key are computed from the job data
						let (action_name, group_key) = job.get_meta();

						// if the job is running, use the in-memory report
						let memory_job = active_reports.values().find(|j| j.id == job.id);
						let report = match memory_job {
							Some(j) => j,
							None => &job,
						};
						// if we have a group key, handle grouping
						if let Some(group_key) = group_key {
							match groups.entry(group_key) {
								// Create new job group with metadata
								Entry::Vacant(e) => {
									let id = job.parent_id.unwrap_or(job.id);
									let group = JobGroup {
										id: id.to_string(),
										action: action_name.clone(),
										status: job.status,
										jobs: vec![report.clone()],
										created_at: job.created_at.unwrap_or(Utc::now()),
									};
									e.insert(group);
								}
								// Add to existing job group
								Entry::Occupied(mut e) => {
									let group = e.get_mut();
									group.jobs.insert(0, report.clone()); // inserts at the beginning
								}
							}
						}
					}

					let mut groups_vec: Vec<JobGroup> = groups.into_values().collect();
					groups_vec.sort_by(|a, b| b.created_at.cmp(&a.created_at));

					// Update the index after sorting the groups
					let mut index: HashMap<String, i32> = HashMap::new();
					for (i, group) in groups_vec.iter().enumerate() {
						for job in &group.jobs {
							index.insert(job.id.clone().to_string(), i as i32);
						}
					}

					Ok(JobGroups {
						groups: groups_vec,
						index,
					})
				})
		})
		.procedure("isActive", {
			R.with2(library()).query(|(ctx, _), _: ()| async move {
				Ok(!ctx.jobs.get_running_reports().await.is_empty())
			})
		})
		.procedure("clear", {
			R.with2(library())
				.mutation(|(_, library), id: Uuid| async move {
					library
						.db
						.job()
						.delete(job::id::equals(id.as_bytes().to_vec()))
						.exec()
						.await?;

					invalidate_query!(library, "jobs.reports");
					Ok(())
				})
		})
		.procedure("clearAll", {
			R.with2(library())
				.mutation(|(_, library), _: ()| async move {
					library.db.job().delete_many(vec![]).exec().await?;

					invalidate_query!(library, "jobs.reports");
					Ok(())
				})
		})
		// pause job
		.procedure("pause", {
			R.with2(library())
				.mutation(|(ctx, _), id: Uuid| async move {
					JobManager::pause(&ctx.jobs, id).await.map_err(Into::into)
				})
		})
		.procedure("resume", {
			R.with2(library())
				.mutation(|(ctx, _), id: Uuid| async move {
					JobManager::resume(&ctx.jobs, id).await.map_err(Into::into)
				})
		})
		.procedure("generateThumbsForLocation", {
			#[derive(Type, Deserialize)]
			pub struct GenerateThumbsForLocationArgs {
				pub id: location::id::Type,
				pub path: PathBuf,
			}

			R.with2(library()).mutation(
				|(_, library), args: GenerateThumbsForLocationArgs| async move {
					let Some(location) = find_location(&library, args.id).exec().await? else {
						return Err(LocationError::IdNotFound(args.id).into());
					};

					library
						.spawn_job(ThumbnailerJobInit {
							location,
							sub_path: Some(args.path),
						})
						.await
						.map_err(Into::into)
				},
			)
		})
		.procedure("objectValidator", {
			#[derive(Type, Deserialize)]
			pub struct ObjectValidatorArgs {
				pub id: location::id::Type,
				pub path: PathBuf,
			}

			R.with2(library())
				.mutation(|(_, library), args: ObjectValidatorArgs| async move {
					if find_location(&library, args.id).exec().await?.is_none() {
						return Err(LocationError::IdNotFound(args.id).into());
					}

					library
						.spawn_job(ObjectValidatorJobInit {
							location_id: args.id,
							path: args.path,
							background: true,
						})
						.await
						.map_err(Into::into)
				})
		})
		.procedure("identifyUniqueFiles", {
			#[derive(Type, Deserialize)]
			pub struct IdentifyUniqueFilesArgs {
				pub id: location::id::Type,
				pub path: PathBuf,
			}

			R.with2(library())
				.mutation(|(_, library), args: IdentifyUniqueFilesArgs| async move {
					let Some(location) = find_location(&library, args.id).exec().await? else {
						return Err(LocationError::IdNotFound(args.id).into());
					};

					library
						.spawn_job(FileIdentifierJobInit {
							location,
							sub_path: Some(args.path),
						})
						.await
						.map_err(Into::into)
				})
		})
		.procedure("newThumbnail", {
			R.with2(library())
				.subscription(|(ctx, _), _: ()| async move {
					// TODO: Only return event for the library that was subscribed to

					let mut event_bus_rx = ctx.event_bus.0.subscribe();
					async_stream::stream! {
						while let Ok(event) = event_bus_rx.recv().await {
							match event {
								CoreEvent::NewThumbnail { thumb_key } => yield thumb_key,
								_ => {}
							}
						}
					}
				})
		})
}
