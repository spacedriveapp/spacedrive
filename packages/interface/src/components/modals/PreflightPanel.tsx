import { CircleNotch, Info, Warning, XCircle } from "@phosphor-icons/react";
import clsx from "clsx";
import type {
	ChangeKind,
	Finding,
	FsPlan,
	FsPlanSummary,
	PlannedChange,
	SdPath,
	Validation,
} from "@sd/ts-client";
import { formatBytes } from "../../routes/explorer/utils";

/**
 * What validation and preview answered for an operation, rendered the same way in
 * every dialog: the findings with their severity, the facts of the
 * execution, and the plan's counts with the rows a person reads a plan for.
 */
export function PreflightPanel({
	validation,
	validating,
	plan,
	planning,
	planError,
}: {
	validation: Validation | undefined;
	validating: boolean;
	plan: FsPlan | undefined;
	planning: boolean;
	planError?: unknown;
}) {
	return (
		<div className="space-y-3">
			<Findings validation={validation} validating={validating} />
			{planning && !plan ? (
				<Pending>Reading the plan from the index…</Pending>
			) : plan ? (
				<PlanSummaryView plan={plan} />
			) : planError ? (
				<div className="text-xs text-ink-dull">
					No plan: {planError instanceof Error ? planError.message : String(planError)}
				</div>
			) : null}
		</div>
	);
}

export function hasErrors(validation: Validation | undefined): boolean {
	return validation?.findings.some((finding) => finding.severity === "error") ?? false;
}

function Pending({ children }: { children: React.ReactNode }) {
	return (
		<div className="flex items-center gap-2 text-xs text-ink-dull">
			<CircleNotch className="size-3.5 animate-spin" weight="bold" />
			{children}
		</div>
	);
}

function Findings({
	validation,
	validating,
}: {
	validation: Validation | undefined;
	validating: boolean;
}) {
	if (!validation) {
		return validating ? <Pending>Checking…</Pending> : null;
	}
	const facts = validation.facts;
	const about: string[] = [`runs on ${facts.executes_on}`];
	if (facts.strategy) about.push(`via ${facts.strategy}`);
	if (facts.estimated_files != null && facts.estimated_bytes != null) {
		about.push(`${facts.estimated_files} files, ${formatBytes(facts.estimated_bytes)} estimated`);
	}
	if (facts.free_space_after != null) {
		about.push(
			facts.free_space_after < 0
				? `${formatBytes(-facts.free_space_after)} short on the destination`
				: `${formatBytes(facts.free_space_after)} free after`,
		);
	}
	return (
		<div className="space-y-1.5">
			{validation.findings.map((finding, index) => (
				<FindingRow key={`${finding.code}-${index}`} finding={finding} />
			))}
			<div className={clsx("text-xs text-ink-faint", validating && "opacity-60")}>
				{about.join(" · ")}
			</div>
		</div>
	);
}

function FindingRow({ finding }: { finding: Finding }) {
	const tone =
		finding.severity === "error"
			? { icon: XCircle, className: "bg-red-500/10 border-red-500/20 text-red-500" }
			: finding.severity === "warning"
				? { icon: Warning, className: "bg-amber-500/10 border-amber-500/20 text-amber-500" }
				: { icon: Info, className: "bg-app-box border-app-line text-ink-dull" };
	const Icon = tone.icon;
	return (
		<div className={clsx("flex items-start gap-2 rounded-md border px-2.5 py-2", tone.className)}>
			<Icon className="mt-0.5 size-4 flex-shrink-0" weight="fill" />
			<div className="min-w-0 flex-1">
				<div className="text-xs text-ink">{finding.message}</div>
				<div className="text-[10px] text-ink-faint">
					{finding.code}
					{finding.path && <> · {pathName(finding.path)}</>}
				</div>
			</div>
		</div>
	);
}

function PlanSummaryView({ plan }: { plan: FsPlan }) {
	const summary = plan.summary;
	const rows = summaryRows(summary);
	const basis =
		plan.basis.kind === "index"
			? `from the index of ${plan.basis.revisions.length} ${
					plan.basis.revisions.length === 1 ? "store" : "stores"
				}`
			: plan.basis.kind === "journal"
				? "from the job's journal"
				: `from the archive's directory of ${plan.basis.entries} entries`;
	// The rows a person reads a plan for: conflicts, the last copies a
	// deletion would remove, replacements, then everything else that goes.
	const attention = plan.changes
		.filter((change) => attentionRank(change.change) > 0)
		.sort((a, b) => attentionRank(b.change) - attentionRank(a.change));
	return (
		<div className="space-y-2">
			<div className="text-xs font-medium text-ink-dull">
				Plan, {basis}
				{plan.truncated && " (first 5000 changes listed)"}
			</div>
			<div className="grid grid-cols-[auto_1fr_auto] gap-x-3 gap-y-0.5 text-xs">
				{rows.map((row) => (
					<Row key={row.label} {...row} />
				))}
			</div>
			{attention.length > 0 && (
				<div className="max-h-40 space-y-0.5 overflow-auto rounded-md bg-app-box p-2">
					{attention.slice(0, 50).map((change) => (
						<ChangeRow key={sdPathKey(change.path)} change={change} />
					))}
					{attention.length > 50 && (
						<div className="text-[10px] text-ink-faint">and {attention.length - 50} more</div>
					)}
				</div>
			)}
		</div>
	);
}

function attentionRank(change: ChangeKind): number {
	switch (change.type) {
		case "conflict":
			return 4;
		case "delete":
			return change.last_copy ? 3 : 1;
		case "replace":
			return 2;
		default:
			return 0;
	}
}

function Row({
	label,
	files,
	bytes,
	tone,
}: {
	label: string;
	files: number;
	bytes?: number;
	tone?: "warn" | "danger";
}) {
	return (
		<>
			<div
				className={clsx(
					"text-ink-dull",
					tone === "warn" && files > 0 && "text-amber-500",
					tone === "danger" && files > 0 && "text-red-500",
				)}
			>
				{label}
			</div>
			<div className={clsx("text-right tabular-nums", files === 0 ? "text-ink-faint" : "text-ink")}>
				{files}
			</div>
			<div className="text-right tabular-nums text-ink-faint">
				{bytes != null && bytes > 0 ? formatBytes(bytes) : ""}
			</div>
		</>
	);
}

function summaryRows(summary: FsPlanSummary) {
	return [
		{ label: "Create", files: summary.creates.files, bytes: summary.creates.bytes },
		{ label: "New folders", files: summary.directories_created },
		{ label: "Replace", files: summary.replaces.files, bytes: summary.replaces.bytes, tone: "warn" as const },
		{ label: "Merge into", files: summary.merged_into },
		{ label: "Move", files: summary.moves.files, bytes: summary.moves.bytes },
		{ label: "Delete", files: summary.deletes.files, bytes: summary.deletes.bytes, tone: "danger" as const },
		{
			label: "Skip, same bytes",
			files: summary.skips.duplicates_confirmed.files + summary.skips.duplicate_candidates.files,
			bytes: summary.skips.duplicates_confirmed.bytes + summary.skips.duplicate_candidates.bytes,
		},
		{ label: "Skip, by policy", files: summary.skips.policy.files, bytes: summary.skips.policy.bytes },
		{ label: "Skip, junk", files: summary.skips.junk },
		{ label: "Conflicts", files: summary.conflicts, tone: "danger" as const },
		{ label: "Attributes", files: summary.attributes },
	];
}

function ChangeRow({ change }: { change: PlannedChange }) {
	return (
		<div className="flex items-center gap-2 text-[11px]">
			<span
				className={clsx(
					"flex-shrink-0 rounded px-1 py-px font-medium",
					change.change.type === "conflict" ||
						(change.change.type === "delete" && change.change.last_copy)
						? "bg-red-500/15 text-red-500"
						: "bg-amber-500/15 text-amber-500",
				)}
			>
				{changeLabel(change.change)}
			</span>
			<span className="truncate text-ink-dull" title={pathString(change.path)}>
				{pathName(change.path)}
			</span>
		</div>
	);
}

/** A short word for a change, as a row badge reads it. */
export function changeLabel(change: ChangeKind): string {
	switch (change.type) {
		case "create":
			return "new";
		case "create_directory":
			return "new folder";
		case "replace":
			return change.reason === "newer" ? "replace, newer" : "replace";
		case "merge_into":
			return "merge into";
		case "skip":
			switch (change.reason) {
				case "duplicate_confirmed":
					return "same bytes";
				case "duplicate_candidate":
					return "same bytes, unverified";
				case "junk":
					return "junk";
				case "policy":
					return "kept";
			}
			return "skip";
		case "move":
			return "move";
		case "delete":
			return change.last_copy ? "delete, last copy" : "delete";
		case "conflict":
			switch (change.kind) {
				case "file_vs_directory":
					return "file vs folder";
				case "link_vs_file":
					return "link vs file";
				case "sources":
					return "two sources";
			}
			return "conflict";
		case "set_attributes":
			return "attributes";
	}
}

export function pathString(path: SdPath): string {
	if ("Physical" in path) return path.Physical.path;
	if ("Cloud" in path) return path.Cloud.path;
	return JSON.stringify(path);
}

export function pathName(path: SdPath): string {
	const whole = pathString(path);
	const parts = whole.split("/").filter(Boolean);
	return parts[parts.length - 1] ?? whole;
}

function sdPathKey(path: SdPath): string {
	return pathString(path);
}
