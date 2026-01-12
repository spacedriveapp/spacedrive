import type { JobRenderer, JobRendererProps, JobDetailsRendererProps } from "./index";
import { CopyJobDetails } from "../components/CopyJobDetails";

/**
 * Extract first filename from copy job for display
 */
function extractFirstFileName(job: JobRendererProps["job"]): string {
	const actionInput = job.action_context?.action_input;
	if (
		typeof actionInput === "object" &&
		actionInput !== null &&
		"sources" in actionInput
	) {
		const sources = actionInput.sources;
		if (
			typeof sources === "object" &&
			sources !== null &&
			"paths" in sources &&
			Array.isArray(sources.paths) &&
			sources.paths.length > 0
		) {
			const firstPath = sources.paths[0];
			if (
				typeof firstPath === "object" &&
				firstPath !== null &&
				"Local" in firstPath
			) {
				const local = firstPath.Local;
				if (typeof local === "object" && local !== null && "path" in local) {
					const path = String(local.path);
					return path.split("/").pop() || path;
				}
			}
		}
	}
	return "items";
}

/**
 * Format bytes to human readable
 */
function formatBytes(bytes: number): string {
	const units = ["B", "KB", "MB", "GB", "TB"];
	let size = bytes;
	let unitIndex = 0;

	while (size >= 1024 && unitIndex < units.length - 1) {
		size /= 1024;
		unitIndex++;
	}

	return `${size.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

/**
 * Format speed (bytes/sec) to human readable
 */
function formatSpeed(bytesPerSecond: number): string {
	return `${formatBytes(bytesPerSecond)}/s`;
}

/**
 * Format duration from seconds to human readable
 */
function formatDurationSeconds(seconds: number): string {
	const mins = Math.floor(seconds / 60);
	const hrs = Math.floor(mins / 60);

	if (hrs > 0) {
		return `${hrs}h ${mins % 60}m`;
	}
	if (mins > 0) {
		return `${mins}m`;
	}
	return `${Math.floor(seconds)}s`;
}

/**
 * Custom card content for file copy jobs
 * Returns ONLY the content (title + subtext), JobCard provides the container
 */
function FileCopyCardContent({ job }: JobRendererProps) {
	const generic = job.generic_progress;
	const metadata = generic?.metadata as any;
	const strategyDescription = metadata?.strategy?.strategy_description || null;
	const isCrossDevice = metadata?.strategy?.is_cross_device || false;
	const isCrossVolume = metadata?.strategy?.is_cross_volume || false;

	// Determine if this is a move operation
	const isMove = job.action_context?.action_type === "files.move";

	// Calculate title
	const fileCount = generic?.completion?.total || 0;
	const fileName = extractFirstFileName(job);
	const title =
		fileCount > 1
			? `${isMove ? "Moving" : "Copying"} ${fileCount} items`
			: `${isMove ? "Moving" : "Copying"} '${fileName}'`;

	// Calculate rich subtext with progress, speed, and ETA
	const completed = generic?.completion?.completed || 0;
	const speed = generic?.performance?.rate ? formatSpeed(generic.performance.rate) : null;
	const eta = generic?.performance?.estimated_remaining
		? formatDurationSeconds(generic.performance.estimated_remaining.secs)
		: null;

	const subtext =
		job.status === "running" && speed && eta
			? `${completed}/${fileCount} files • ${speed} • ${eta} remaining`
			: job.status === "running"
			? `${completed}/${fileCount} files`
			: job.status === "completed"
			? "Completed"
			: job.status === "failed"
			? "Failed"
			: job.status === "paused"
			? "Paused"
			: "Preparing...";

	return (
		<div className="flex-1 flex flex-col gap-1.5 min-h-0">
			{/* Title with badges */}
			<div className="flex items-center gap-2 min-h-0">
				<span className="truncate text-[13px] font-medium text-ink">
					{title}
				</span>

				{/* Transfer method badges */}
				{strategyDescription && (
					<span className="px-1.5 py-0.5 text-[9px] font-medium text-ink-faint bg-app-darkBox rounded-md truncate max-w-[120px]">
						{strategyDescription}
					</span>
				)}
				{isCrossDevice && (
					<span className="px-1.5 py-0.5 text-[9px] font-medium text-accent bg-accent/10 rounded-md whitespace-nowrap">
						Cross-device
					</span>
				)}
				{!isCrossDevice && isCrossVolume && (
					<span className="px-1.5 py-0.5 text-[9px] font-medium text-ink-faint bg-app-darkBox rounded-md whitespace-nowrap">
						Cross-volume
					</span>
				)}
			</div>

			{/* Rich subtext */}
			<div className="min-h-0">
				<span
					className="text-[10px] text-ink-dull truncate block"
					style={{ opacity: 0.7 }}
				>
					{subtext}
				</span>
			</div>
		</div>
	);
}

/**
 * Details panel for expanded file copy jobs
 */
function FileCopyDetailsPanel({ job, speedHistory }: JobDetailsRendererProps) {
	return <CopyJobDetails job={job} speedHistory={speedHistory} />;
}

/**
 * File copy job renderer with rich UI
 */
export const FileCopyRenderer: JobRenderer = {
	CardContent: FileCopyCardContent,
	DetailsPanel: FileCopyDetailsPanel,
};
