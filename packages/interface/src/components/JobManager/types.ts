import type { JobStatus, JobListItem as GeneratedJobListItem, JsonValue, SdPath } from "@sd/ts-client";

// GenericProgress type from Rust (matches core/src/infra/job/generic_progress.rs)
export interface GenericProgress {
	percentage: number;
	phase: string;
	current_path?: SdPath;
	message: string;
	completion: {
		completed: number;
		total: number;
		bytes_completed?: number;
		total_bytes?: number;
	};
	performance: {
		rate: number;
		estimated_remaining?: { secs: number; nanos: number };
		elapsed?: { secs: number; nanos: number };
		error_count: number;
		warning_count: number;
	};
	metadata: any;
}

// Extend the generated type with runtime fields from JobProgress events
export type JobListItem = GeneratedJobListItem & {
	current_phase?: string;
	current_path?: SdPath;
	status_message?: string;
	generic_progress?: GenericProgress;
};

export const JOB_STATUS_COLORS = {
  running: "rgb(0, 122, 255)",
  completed: "rgb(52, 199, 89)",
  failed: "rgb(255, 59, 48)",
  paused: "rgb(255, 149, 0)",
  queued: "rgba(255, 255, 255, 0.5)",
  cancelled: "rgb(255, 59, 48)",
} as const;

export const CARD_HEIGHT = 72; // px
export const STATUS_DOT_SIZE = 8; // px
export const PROGRESS_BAR_HEIGHT = 12; // px

/**
 * Extracts a meaningful display name from the job's action context
 */
export function getJobDisplayName(job: JobListItem): string {
  if (!job.action_context?.action_type) {
    // Fallback to capitalizing job name
    if (job.name === "indexer") {
      return "Indexing";
    }
    if (job.name === "thumbnail_generation") {
      return "Generating Thumbnails";
    }
    return job.name.split("_").map(word =>
      word.charAt(0).toUpperCase() + word.slice(1)
    ).join(" ");
  }

  const { action_type, action_input } = job.action_context;

  try {
    switch (action_type) {
      case "locations.add": {
        const path = extractPath(action_input);
        if (path) {
          // Show full path with ~ for home directory
          const homePath = path.replace(/^\/Users\/[^/]+/, "~");
          return `Added location ${homePath}`;
        }
        break;
      }
      case "files.copy": {
        const source = extractSourcePath(action_input);
        if (source) {
          const fileName = source.split("/").pop() || source;
          return `Copying '${fileName}'`;
        }
        break;
      }
      case "files.move": {
        const source = extractSourcePath(action_input);
        if (source) {
          const fileName = source.split("/").pop() || source;
          return `Moving '${fileName}'`;
        }
        break;
      }
      case "files.delete": {
        const target = extractTargetPath(action_input);
        if (target) {
          const fileName = target.split("/").pop() || target;
          return `Deleting '${fileName}'`;
        }
        break;
      }
      case "media.thumbnail":
        return "Generating Thumbnails";
      case "media.extract":
        return "Extracting Media";
      case "indexing.start":
        return "Indexing Location";
      default: {
        // Capitalize and format action type
        return action_type
          .split(".")
          .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
          .join(" ");
      }
    }
  } catch (error) {
    console.warn("Failed to extract display name from action context:", error);
  }

  return job.name;
}

/**
 * Gets the subtext to display below the job title
 */
export function getJobSubtext(job: JobListItem): string {
  switch (job.status) {
    case "running": {
      // Use rich metadata from JobProgress events if available
      if (job.status_message) return job.status_message;
      if (job.current_phase) return job.current_phase;
      if (job.current_path) {
        const pathStr = typeof job.current_path === 'string'
          ? job.current_path
          : JSON.stringify(job.current_path);
        return pathStr;
      }
      return job.progress > 0 ? `${Math.round(job.progress * 100)}%` : "Processing...";
    }
    case "completed":
      return "Completed";
    case "failed":
      return "Job failed";
    case "queued":
      return "Waiting to start";
    case "paused":
      return "Paused";
    case "cancelled":
      return "Cancelled";
    default:
      return "";
  }
}

/**
 * Gets the status badge text (shown on the right side)
 */
export function getStatusBadge(job: JobListItem): string {
  switch (job.status) {
    case "running":
      return `${Math.round(job.progress * 100)}%`;
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "paused":
      return "Paused";
    case "queued":
      return "Queued";
    case "cancelled":
      return "Cancelled";
    default:
      return "";
  }
}

/**
 * Formats duration in milliseconds to human-readable string
 */
export function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  }
  return `${seconds}s`;
}

/**
 * Formats a date to time ago (e.g., "2m ago", "1h ago")
 */
export function timeAgo(date: string | Date | undefined): string {
  if (!date) return "—";

  const now = new Date();
  const past = typeof date === "string" ? new Date(date) : date;

  // Check if date is valid
  if (isNaN(past.getTime())) return "—";

  const diffMs = now.getTime() - past.getTime();
  const diffSeconds = Math.floor(diffMs / 1000);
  const diffMinutes = Math.floor(diffSeconds / 60);
  const diffHours = Math.floor(diffMinutes / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffDays > 0) return `${diffDays}d ago`;
  if (diffHours > 0) return `${diffHours}h ago`;
  if (diffMinutes > 0) return `${diffMinutes}m ago`;
  return "just now";
}

// Helper functions to extract paths from JsonValue
function extractPath(input: JsonValue): string | null {
  if (typeof input === "object" && input !== null && "path" in input) {
    const path = input.path;
    // Handle Physical path: { Physical: { device_slug: "...", path: "..." } }
    if (typeof path === "object" && path !== null && "Physical" in path) {
      const physical = path.Physical;
      if (typeof physical === "object" && physical !== null && "path" in physical) {
        return String(physical.path);
      }
    }
    // Handle Local path: { Local: { path: "..." } }
    if (typeof path === "object" && path !== null && "Local" in path) {
      const local = path.Local;
      if (typeof local === "object" && local !== null && "path" in local) {
        return String(local.path);
      }
    }
    // Handle direct string path
    if (typeof path === "string") {
      return path;
    }
  }
  return null;
}

function extractSourcePath(input: JsonValue): string | null {
  if (typeof input === "object" && input !== null && "source" in input) {
    return String(input.source);
  }
  if (typeof input === "object" && input !== null && "sources" in input) {
    const sources = input.sources;
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
          return String(local.path);
        }
      }
    }
  }
  return null;
}

function extractTargetPath(input: JsonValue): string | null {
  if (typeof input === "object" && input !== null && "target" in input) {
    return String(input.target);
  }
  if (typeof input === "object" && input !== null && "targets" in input) {
    const targets = input.targets;
    if (
      typeof targets === "object" &&
      targets !== null &&
      "paths" in targets &&
      Array.isArray(targets.paths) &&
      targets.paths.length > 0
    ) {
      return String(targets.paths[0]);
    }
  }
  return null;
}
