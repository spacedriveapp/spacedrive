//! # Validate and preview
//!
//! Two questions an operation answers before it runs, both over its exact
//! input, so nothing drifts between what a client was shown and what it
//! dispatches. Validation says whether and how the operation would run:
//! findings with stable codes and the facts of its execution, read from
//! registries and rollups so a dialog can ask again as every option changes.
//! Preview says what would exist afterward, projected from the index at a
//! cost proportional to what the operation touches. Both are reads: the
//! [`PreviewContext`] hands out the index, the stores through it, the volume
//! registry and the library, and nothing that writes.
//!
//! Errors refuse, warnings inform. An action that registered a validator is
//! validated again when it is dispatched, and an error finding stops it
//! there, with the findings handed back as [`Validation::refusal`]. A warning
//! never blocks; the conversation about warnings belongs to the client, and
//! dispatching anyway is the confirmation. A preview is advisory: the
//! filesystem is live, so a job applies the same policy per leaf when it runs
//! and reports where it diverged from the plan.
//!
//! Both are opt-in traits registered beside the action:
//! `register_validate!(FileMergeAction, "files.merge")` puts
//! `validate:files.merge` on the wire and `register_preview!` puts
//! `preview:files.merge` there, each taking the action's input.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{error::ActionError, LibraryAction};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::api::SessionContext,
	library::Library,
	ops::{files::plan::PlanHandles, indexing::VolumeIndex},
	volume::VolumeManager,
};

/// Answers whether and how a library action would run, from its exact input.
pub trait ValidatedAction: LibraryAction {
	fn validate(
		input: &Self::Input,
		ctx: &PreviewContext,
	) -> impl std::future::Future<Output = Result<Validation, ActionError>> + Send;
}

/// Answers what would exist after a library action, from its exact input.
pub trait PreviewableAction: LibraryAction {
	/// The projection, typed per action. Filesystem-mutating actions share
	/// one plan type; others bring their own.
	type Plan: Serialize + Send + 'static;

	fn preview(
		input: Self::Input,
		ctx: &PreviewContext,
	) -> impl std::future::Future<Output = Result<Self::Plan, ActionError>> + Send;
}

/// What a preflight check reads from: the index and the stores through it, the volume
/// registry, and the library. Nothing here writes, which is what lets a
/// client ask both questions as often as it likes.
pub struct PreviewContext {
	index: Arc<VolumeIndex>,
	volumes: Arc<VolumeManager>,
	library: Arc<Library>,
	session: SessionContext,
	plans: Arc<PlanHandles>,
}

impl PreviewContext {
	pub fn new(context: &CoreContext, library: Arc<Library>, session: SessionContext) -> Self {
		Self {
			index: context.volume_index().clone(),
			volumes: context.volume_manager.clone(),
			library,
			session,
			plans: context.plans.clone(),
		}
	}

	/// The plans the daemon keeps under handles. Retaining a plan there is
	/// what lets a client browse it as an overlay; it writes nothing.
	pub fn plans(&self) -> &PlanHandles {
		&self.plans
	}

	pub fn index(&self) -> &VolumeIndex {
		&self.index
	}

	pub fn volumes(&self) -> &VolumeManager {
		&self.volumes
	}

	pub fn library(&self) -> &Library {
		&self.library
	}

	pub fn session(&self) -> &SessionContext {
		&self.session
	}

	/// The device an execution would run on: the one answering.
	pub fn executes_on(&self) -> String {
		crate::device::get_current_device_slug()
	}

	/// The stores beneath a path on this device, which is what preflight reads
	/// a folder from.
	pub async fn reach(&self, path: &SdPath) -> Vec<crate::ops::paths::reach::Reach> {
		crate::ops::paths::reach::stores_beneath_in(&self.volumes, &self.index, path).await
	}
}

/// Whether and how an action would run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct Validation {
	pub findings: Vec<Finding>,
	pub facts: ExecutionFacts,
}

/// One thing validation has to say, with a stable code a client or an agent
/// can branch on rather than parsing the message.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Finding {
	pub severity: Severity,
	pub code: String,
	pub message: String,
	/// The path the finding is about, when it is about one.
	pub path: Option<SdPath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
	/// Refuses the execution.
	Error,
	/// Worth showing; never blocks.
	Warning,
	Info,
}

/// How the execution would run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ExecutionFacts {
	/// The device slug the work runs on.
	pub executes_on: String,
	/// The strategy the router would pick: reflink, atomic rename, stream,
	/// remote.
	pub strategy: Option<String>,
	pub estimated_files: Option<u64>,
	pub estimated_bytes: Option<u64>,
	pub free_space_after: Option<i64>,
}

/// What a refused dispatch's error starts with. The wire's error channel is
/// a string, so the findings travel as JSON behind this prefix and a client
/// tells a refusal from any other failure by it.
pub const REFUSED: &str = "refused:";

impl Validation {
	pub fn errors(&self) -> impl Iterator<Item = &Finding> {
		self.findings
			.iter()
			.filter(|finding| finding.severity == Severity::Error)
	}

	/// Whether an execution over this input is refused.
	pub fn refuses(&self) -> bool {
		self.errors().next().is_some()
	}

	/// The error a refused dispatch answers with: the validation as JSON,
	/// so the client reads the findings back with [`Self::from_refusal`].
	pub fn refusal(&self) -> String {
		// A validation is plain data; serializing it cannot fail.
		format!(
			"{REFUSED}{}",
			serde_json::to_string(self).unwrap_or_default()
		)
	}

	/// The validation a refused dispatch carried, from its error.
	pub fn from_refusal(error: &str) -> Option<Self> {
		serde_json::from_str(error.strip_prefix(REFUSED)?).ok()
	}
}

impl Finding {
	pub fn new(severity: Severity, code: impl Into<String>, message: impl Into<String>) -> Self {
		Self {
			severity,
			code: code.into(),
			message: message.into(),
			path: None,
		}
	}

	pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
		Self::new(Severity::Error, code, message)
	}

	pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
		Self::new(Severity::Warning, code, message)
	}

	pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
		Self::new(Severity::Info, code, message)
	}

	pub fn at(mut self, path: SdPath) -> Self {
		self.path = Some(path);
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::daemon::rpc::execute_json_operation_with_context;
	use serde_json::json;
	use std::{path::PathBuf, sync::Mutex};

	/// The tags of the probe inputs that reached execution.
	static RUNS: Mutex<Vec<String>> = Mutex::new(Vec::new());

	/// An action that answers both preflight methods, so they can be
	/// exercised over the wire without a real action.
	#[derive(Debug, Clone, Serialize, Deserialize, Type)]
	struct ProbeInput {
		/// Whether validation raises an error finding.
		refuse: bool,
		/// A source root the preview reads the store of.
		source: Option<PathBuf>,
		/// Names the input in [`RUNS`].
		tag: String,
	}

	#[derive(Debug, Clone, Serialize, Deserialize, Type)]
	struct ProbeOutput {
		ran: bool,
	}

	#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
	struct ProbePlan {
		sources: usize,
		/// Whether the source's store holds a path that is not there, which
		/// is a read of the store; `None` without a source.
		holds: Option<bool>,
	}

	struct ProbeAction {
		tag: String,
	}

	impl LibraryAction for ProbeAction {
		type Input = ProbeInput;
		type Output = ProbeOutput;

		fn from_input(input: ProbeInput) -> Result<Self, String> {
			Ok(Self { tag: input.tag })
		}

		async fn execute(
			self,
			_library: Arc<Library>,
			_context: Arc<CoreContext>,
		) -> Result<ProbeOutput, ActionError> {
			RUNS.lock().expect("runs").push(self.tag);
			Ok(ProbeOutput { ran: true })
		}

		fn action_kind(&self) -> &'static str {
			"probe.preflight"
		}
	}

	impl ValidatedAction for ProbeAction {
		async fn validate(
			input: &ProbeInput,
			ctx: &PreviewContext,
		) -> Result<Validation, ActionError> {
			let finding = if input.refuse {
				Finding::error("probe.refused", "the probe was told to refuse")
			} else {
				Finding::info("probe.fine", "nothing stands in the way")
			};
			Ok(Validation {
				findings: vec![finding],
				facts: ExecutionFacts {
					executes_on: ctx.executes_on(),
					..Default::default()
				},
			})
		}
	}

	impl PreviewableAction for ProbeAction {
		type Plan = ProbePlan;

		async fn preview(
			input: ProbeInput,
			ctx: &PreviewContext,
		) -> Result<ProbePlan, ActionError> {
			let holds = match &input.source {
				Some(root) => match ctx.index().store_for(root).await {
					Some(store) => Some(store.contains_path(&root.join("nothing")).await),
					None => None,
				},
				None => None,
			};
			Ok(ProbePlan {
				sources: ctx.index().sources().len(),
				holds,
			})
		}
	}

	crate::register_library_action!(ProbeAction, "probe.preflight");
	crate::register_validate!(ProbeAction, "probe.preflight");
	crate::register_preview!(ProbeAction, "probe.preflight");

	async fn core_with_library() -> (tempfile::TempDir, crate::Core, Arc<Library>) {
		let data = tempfile::tempdir().expect("tempdir");
		let core = crate::Core::new(data.path().to_path_buf())
			.await
			.expect("core");
		let library = core
			.libraries
			.create_library("Preflight", None, core.context.clone())
			.await
			.expect("library");
		(data, core, library)
	}

	/// One call as the daemon socket, an embedded host or a paired device
	/// would make it.
	async fn call(
		core: &crate::Core,
		library: &Library,
		method: &str,
		payload: serde_json::Value,
	) -> Result<serde_json::Value, String> {
		execute_json_operation_with_context(method, Some(library.id()), payload, &core.context)
			.await
	}

	fn ran(tag: &str) -> bool {
		RUNS.lock().expect("runs").iter().any(|run| run == tag)
	}

	/// Both preflight methods answer over the wire for the input the action takes, and
	/// the action then runs on that input unchanged.
	#[tokio::test]
	async fn the_rails_answer_over_the_wire_for_the_actions_input() {
		let (_data, core, library) = core_with_library().await;
		let input = json!({"refuse": false, "source": null, "tag": "answers"});

		let validation: Validation = serde_json::from_value(
			call(&core, &library, "validate:probe.preflight", input.clone())
				.await
				.expect("validated"),
		)
		.expect("a validation");
		assert_eq!(validation.findings[0].code, "probe.fine");
		assert!(!validation.refuses());
		assert_eq!(
			validation.facts.executes_on,
			crate::device::get_current_device_slug()
		);

		let plan: ProbePlan = serde_json::from_value(
			call(&core, &library, "preview:probe.preflight", input.clone())
				.await
				.expect("previewed"),
		)
		.expect("a plan");
		assert_eq!(plan.holds, None);

		let output: ProbeOutput = serde_json::from_value(
			call(&core, &library, "action:probe.preflight.input", input)
				.await
				.expect("dispatched"),
		)
		.expect("an output");
		assert!(output.ran);
		assert!(ran("answers"));
	}

	/// An error finding refuses the dispatch on the server, before the action
	/// runs, and the findings come back structured.
	#[tokio::test]
	async fn an_error_finding_refuses_the_dispatch() {
		let (_data, core, library) = core_with_library().await;
		let error = call(
			&core,
			&library,
			"action:probe.preflight.input",
			json!({"refuse": true, "source": null, "tag": "refused"}),
		)
		.await
		.expect_err("refused");

		let validation = Validation::from_refusal(&error).expect("the findings, structured");
		assert_eq!(
			validation
				.errors()
				.map(|finding| finding.code.as_str())
				.collect::<Vec<_>>(),
			["probe.refused"]
		);
		assert!(!ran("refused"));
	}

	/// Preflight reads a store without writing it: the store's revision is what
	/// it was.
	#[tokio::test]
	async fn the_rails_write_nothing() {
		let (_data, core, library) = core_with_library().await;
		let files = tempfile::tempdir().expect("tempdir");
		let root = files.path().canonicalize().expect("root");
		let cache = core.context.volume_index();
		cache
			.register_source(&root, None)
			.await
			.expect("registered");
		let store = cache.store_for(&root).await.expect("a store");
		let before = store.db().revision().await.expect("revision");

		let input = json!({"refuse": false, "source": root, "tag": "reads"});
		call(&core, &library, "validate:probe.preflight", input.clone())
			.await
			.expect("validated");
		let plan: ProbePlan = serde_json::from_value(
			call(&core, &library, "preview:probe.preflight", input)
				.await
				.expect("previewed"),
		)
		.expect("a plan");
		assert_eq!(plan.holds, Some(false), "the preview read the store");
		assert!(plan.sources >= 1);

		assert_eq!(store.db().revision().await.expect("revision"), before);
	}
}
