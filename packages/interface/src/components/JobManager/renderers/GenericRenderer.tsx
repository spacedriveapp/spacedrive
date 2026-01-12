import type { JobRenderer, JobRendererProps } from "./index";
import { getJobDisplayName, getJobSubtext } from "../types";

/**
 * Generic job card renderer - used for all jobs without custom renderers
 * Maintains the current display logic for backward compatibility
 */
function GenericCardContent({ job }: JobRendererProps) {
	const displayName = getJobDisplayName(job);
	const subtext = getJobSubtext(job);

	return (
		<div className="flex-1 flex flex-col gap-2 min-h-0">
			{/* Title */}
			<div className="flex items-center gap-3 min-h-0">
				<span className="flex-1 truncate text-[13px] font-medium text-ink">
					{displayName}
				</span>
			</div>

			{/* Subtext */}
			<div className="min-h-0">
				<span
					className="text-[10px] text-ink-dull max-w-[200px] truncate block"
					style={{ opacity: 0.7 }}
				>
					{subtext}
				</span>
			</div>
		</div>
	);
}

/**
 * Generic renderer with no details panel
 */
export const GenericRenderer: JobRenderer = {
	CardContent: GenericCardContent,
	// No DetailsPanel - generic jobs don't expand
};
