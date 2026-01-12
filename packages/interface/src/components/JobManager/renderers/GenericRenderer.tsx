import { Pause, Play, X, CaretDown } from "@phosphor-icons/react";
import { motion } from "framer-motion";
import type { JobRenderer, JobRendererProps } from "./index";
import { getJobDisplayName, getJobSubtext } from "../types";

/**
 * Generic job card renderer - used for all jobs without custom renderers
 * Maintains the current display logic for backward compatibility
 */
function GenericCardContent({
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
	const displayName = getJobDisplayName(job);
	const subtext = getJobSubtext(job);

	return (
		<>
			{/* Row 1: Title, status badge, and controls */}
			<div className="flex items-center gap-3 min-h-0">
				<span className="flex-1 truncate text-[13px] font-medium text-ink">
					{displayName}
				</span>

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

			{/* Row 2: Subtext */}
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
 * Generic renderer with no details panel
 */
export const GenericRenderer: JobRenderer = {
	CardContent: GenericCardContent,
	// No DetailsPanel - generic jobs don't expand
};
