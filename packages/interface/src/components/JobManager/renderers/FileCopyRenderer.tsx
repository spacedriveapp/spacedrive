import { Pause, Play, X, CaretDown } from "@phosphor-icons/react";
import { motion } from "framer-motion";
import type { JobRenderer, JobRendererProps, JobDetailsRendererProps } from "./index";
import { CopyJobDetails } from "../components/CopyJobDetails";
import { useNormalizedQuery } from "../../../contexts/SpacedriveContext";
import type { Device } from "@sd/ts-client";

/**
 * Map strategy name to display label (enables i18n in future)
 */
function getStrategyLabel(strategyName: string | undefined, isMove: boolean): string | null {
	if (!strategyName) return null;

	switch (strategyName) {
		case "RemoteTransfer":
			return "Network";
		case "LocalMove":
			return "Atomic";
		case "FastCopy":
			return "Fast";
		case "LocalStream":
			return "Streaming";
		default:
			return strategyName;
	}
}

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
 * Renders the full card content: title row + subtext row
 */
function FileCopyCardContent({
	job,
	isExpanded,
	statusBadge,
	canExpand,
	isHovered,
	showActionButton,
	canPause,
	canResume,
	canCancel,
	onAction,
	onCancel,
}: JobRendererProps) {
	const generic = job.generic_progress;
	const metadata = generic?.metadata as any;
	const strategyName = metadata?.strategy?.strategy_name;
	const strategyLabel = getStrategyLabel(strategyName, job.action_context?.action_type === "files.move");

	// Fetch devices to determine if destination is remote
	const { data: devices } = useNormalizedQuery<any, Device[]>({
		wireMethod: "query:devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	// Determine if this is a move operation
	const isMove = job.action_context?.action_type === "files.move";

	// Check if this is a cross-device transfer from metadata
	const isCrossDevice = metadata?.strategy?.is_cross_device === true;

	// Find current device and infer destination device
	const currentDevice = devices?.find(d => d.is_current);

	// For cross-device transfers, the destination is the device that's NOT current
	// (assuming only 2 devices in the transfer scenario)
	const destinationDevice = isCrossDevice && currentDevice
		? devices?.find(d => !d.is_current)
		: null;

	// Calculate title
	const fileCount = generic?.completion?.total || 0;
	const fileName = extractFirstFileName(job);
	const baseTitle = fileCount > 1
		? `${isMove ? "Moving" : "Copying"} ${fileCount} items`
		: `${isMove ? "Moving" : "Copying"} '${fileName}'`;

	const title = destinationDevice
		? `${baseTitle} to ${destinationDevice.name}`
		: baseTitle;

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
		<>
			{/* Row 1: Title, badges, status badge, and controls */}
			<div className="flex items-center gap-3 min-h-0">
				<span className="flex-1 truncate text-[13px] font-medium text-ink">
					{title}
				</span>

				{/* Transfer method badge */}
				{strategyLabel && (
					<span className="px-1.5 py-0.5 text-[9px] font-medium text-ink-faint bg-app-darkBox rounded-md whitespace-nowrap">
						{strategyLabel}
					</span>
				)}

				{/* Status badge */}
				<span className="flex-shrink-0 text-[11px] font-medium text-ink-dull max-w-[80px] truncate">
					{statusBadge}
				</span>

				{/* Expansion caret */}
				{canExpand && (
					<motion.div
						animate={{ rotate: isExpanded ? 180 : 0 }}
						transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
						className="flex-shrink-0"
					>
						<CaretDown size={12} weight="bold" className="text-ink-dull" />
					</motion.div>
				)}

				{/* Action buttons */}
				{isHovered && (
					<div className="flex items-center gap-1">
						{showActionButton && (canPause || canResume) && (
							<button
								onClick={onAction}
								className="flex-shrink-0 flex items-center justify-center w-4 h-4 rounded-full bg-app-hover hover:bg-app-selected transition-colors"
								title={canPause ? "Pause job" : "Resume job"}
							>
								{canPause ? (
									<Pause size={10} weight="fill" className="text-ink" />
								) : (
									<Play size={10} weight="fill" className="text-ink" />
								)}
							</button>
						)}
						{canCancel && (
							<button
								onClick={onCancel}
								className="flex-shrink-0 flex items-center justify-center w-4 h-4 rounded-full bg-app-hover hover:bg-red-500 transition-colors"
								title="Cancel job"
							>
								<X size={10} weight="bold" className="text-ink hover:text-white" />
							</button>
						)}
					</div>
				)}
			</div>

			{/* Row 2: Rich subtext */}
			<div className="min-h-0">
				<span
					className="text-[10px] text-ink-dull max-w-[200px] truncate block"
					style={{ opacity: 0.7 }}
				>
					{subtext}
				</span>
			</div>
		</>
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
