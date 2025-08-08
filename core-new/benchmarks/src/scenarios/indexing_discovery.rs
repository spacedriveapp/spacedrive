use anyhow::Result;
use regex::Regex;
use std::path::PathBuf;
use std::sync::Arc;

use crate::core_boot::CoreBoot;
use crate::metrics::ScenarioResult;
use crate::recipe::Recipe;
use super::Scenario;

#[derive(Default)]
pub struct IndexingDiscoveryScenario {
    job_ids: Vec<uuid::Uuid>,
    job_logs_dir: Option<PathBuf>,
    library: Option<Arc<sd_core_new::library::Library>>,
}

#[async_trait::async_trait]
impl Scenario for IndexingDiscoveryScenario {
    fn name(&self) -> &'static str { "indexing-discovery" }

    fn describe(&self) -> &'static str {
        "Index volumes and report discovery throughput"
    }

    async fn prepare(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<()> {
        use sd_core_new::infrastructure::actions::handler::ActionHandler;
        // Get or create primary library
        let core = &boot.core;
        let context = core.context.clone();
        let library = match core.libraries.get_primary_library().await {
            Some(lib) => lib,
            None => {
                core.libraries
                    .create_library("Benchmarks", None, context.clone())
                    .await
                    .map_err(|e| anyhow::anyhow!("create benchmark library: {}", e))?
            }
        };
        self.job_logs_dir = Some(boot.job_logs_dir.clone());
        self.library = Some(library.clone());

        // Add locations and collect job IDs
        for loc in &recipe.locations {
            let action = sd_core_new::infrastructure::actions::Action::LocationAdd {
                library_id: library.id(),
                action: sd_core_new::operations::locations::add::action::LocationAddAction {
                    path: loc.path.clone(),
                    name: Some(format!("bench:{}", recipe.name)),
                    mode: sd_core_new::operations::indexing::IndexMode::Shallow,
                },
            };
            let handler = sd_core_new::operations::locations::add::action::LocationAddHandler::new();
            handler.validate(context.clone(), &action).await?;
            let out = handler.execute(context.clone(), action).await?;
            if let sd_core_new::infrastructure::actions::output::ActionOutput::Custom { data, .. } = &out {
                if let Some(j) = data.get("job_id").and_then(|v| v.as_str()) {
                    if let Ok(id) = uuid::Uuid::parse_str(j) {
                        self.job_ids.push(id);
                    }
                }
            }
        }

        Ok(())
    }

    async fn run(&mut self, _boot: &CoreBoot, recipe: &Recipe) -> Result<Vec<ScenarioResult>> {
        let mut results: Vec<ScenarioResult> = Vec::new();

        if self.job_ids.is_empty() || self.library.is_none() {
            return Ok(results);
        }

        let job_manager = self.library.as_ref().unwrap().jobs().clone();
        loop {
            let mut remaining = 0usize;
            for jid in &self.job_ids {
                match job_manager.get_job_info(*jid).await {
                    Ok(Some(info)) => {
                        if !info.status.is_terminal() {
                            remaining += 1;
                        }
                    }
                    Ok(None) => {}
                    Err(_) => remaining += 1,
                }
            }
            if remaining == 0 { break; }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        // Parse metrics from job logs (temporary until structured metrics available)
        let re = Regex::new(r"Indexing completed in ([0-9.]+)s:|Files: ([0-9]+) \(([0-9.]+)/s\)|Directories: ([0-9]+) \(([0-9.]+)/s\)|Total size: ([0-9.]+) GB|Errors: ([0-9]+)").unwrap();
        let log_dir = self.job_logs_dir.clone().unwrap_or_else(|| PathBuf::from("."));

        for jid in &self.job_ids {
            let log_path = log_dir.join(format!("{}.log", jid));
            let mut files = None;
            let mut files_per_s = None;
            let mut dirs = None;
            let mut dirs_per_s = None;
            let mut total_gb = None;
            let mut duration_s = None;
            let mut errors = None;
            if let Ok(txt) = std::fs::read_to_string(&log_path) {
                for cap in re.captures_iter(&txt) {
                    if let Some(d) = cap.get(1) { duration_s = d.as_str().parse::<f64>().ok(); }
                    if let Some(f) = cap.get(2) { files = f.as_str().parse::<u64>().ok(); }
                    if let Some(fp) = cap.get(3) { files_per_s = fp.as_str().parse::<f64>().ok(); }
                    if let Some(di) = cap.get(4) { dirs = di.as_str().parse::<u64>().ok(); }
                    if let Some(dp) = cap.get(5) { dirs_per_s = dp.as_str().parse::<f64>().ok(); }
                    if let Some(ts) = cap.get(6) { total_gb = ts.as_str().parse::<f64>().ok(); }
                    if let Some(e) = cap.get(7) { errors = e.as_str().parse::<u64>().ok(); }
                }
            }
            results.push(ScenarioResult {
                id: *jid,
                scenario: self.name().to_string(),
                recipe_name: recipe.name.clone(),
                duration_s: duration_s.unwrap_or_default(),
                files: files.unwrap_or_default(),
                files_per_s: files_per_s.unwrap_or_default(),
                directories: dirs.unwrap_or_default(),
                directories_per_s: dirs_per_s.unwrap_or_default(),
                total_gb: total_gb.unwrap_or_default(),
                errors: errors.unwrap_or_default(),
                raw_artifacts: vec![log_path],
            });
        }

        Ok(results)
    }
}
