# Rust <> TypeScript dataflow

- All resource types are defined in Rust and codegen'd by prisma-client-rust
- Queries/Mutations are defined in Rust and linked to a `Resource`
- Mutating resources will automatically trigger invalidation of cached data in TypeScript queries using that resource

Flow for changing an active "Job" resource for example:

1. Resource is defined in a prisma schema
   ```prisma
        model Job {
            id                   Int      @id @default(autoincrement())
            client_id            Int
            action               Int
            status               Int      @default(0)
            percentage_complete  Int      @default(0)
            task_count           Int      @default(1)
            completed_task_count Int      @default(0)
            date_created         DateTime @default(now())
            date_modified        DateTime @default(now())
        }
   ```
2. Resolver is defined in Rust
   ```rust
    #[derive(Serialize, Deserialize, Debug)]
    pub struct GetLatestJobs {
        amount: u8
    }

    impl Resolver for GetLatestJobs {
        pub async fn resolve(&self) -> Result<Vec<Job>> {
            let db = db::get().await?;
            let jobs = db.job().find_many(vec![]).exec().await;
            Ok(jobs)
        }
    }
   ```
3. Rust backend dispatches change to a resource
   ```rust
        Commit::new(
            Resource::Job,
            db.job()
                .find_unique(Job::id().equals(job_id.to_string()))
                .update(vec![
                    Job::percentage_complete().set(&percentage_complete),
                    Job::completed_task_count().set(&completed_task_count),
                ])
                .raw(),
        )
        .exec()
        .await?;
    ```
4. From within the