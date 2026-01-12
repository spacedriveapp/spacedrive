import type { JobListItem } from "../types";
import type { SpeedSample } from "../hooks/useJobs";
import { FileCopyRenderer } from "./FileCopyRenderer";
import { GenericRenderer } from "./GenericRenderer";

/**
 * Props passed to job card content renderers
 */
export interface JobRendererProps {
	job: JobListItem;
	isExpanded: boolean;
	statusBadge: string;
	canExpand: boolean;
	isHovered: boolean;
	showActionButton: boolean;
	canPause: boolean;
	canResume: boolean;
	canCancel: boolean;
	onAction: (e: React.MouseEvent) => void;
	onCancel: (e: React.MouseEvent) => void;
}

/**
 * Props passed to job details panel renderers (shown when expanded)
 */
export interface JobDetailsRendererProps {
	job: JobListItem;
	speedHistory: SpeedSample[];
}

/**
 * A job renderer provides custom JSX for a specific job type
 */
export interface JobRenderer {
	/**
	 * Render the collapsed card content (title, subtext, badges, etc.)
	 * This replaces the middle section of the card between status dot and controls
	 */
	CardContent: React.ComponentType<JobRendererProps>;

	/**
	 * Optional: Render expanded details panel
	 * If not provided, no expansion toggle is shown
	 */
	DetailsPanel?: React.ComponentType<JobDetailsRendererProps>;
}

/**
 * Registry of job-specific renderers
 * Add new job types here to customize their display
 */
const jobRenderers: Record<string, JobRenderer> = {
	file_copy: FileCopyRenderer,
	// Add more job types here as needed
};

/**
 * Get the renderer for a specific job type, falling back to generic
 */
export function getJobRenderer(jobName: string): JobRenderer {
	return jobRenderers[jobName] || GenericRenderer;
}
