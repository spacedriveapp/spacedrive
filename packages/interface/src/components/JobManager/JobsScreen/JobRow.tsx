import { Pause, Play, X } from "@phosphor-icons/react";
import clsx from "clsx";
import { useState } from "react";
import { JobStatusIndicator } from "../components/JobStatusIndicator";
import type { JobListItem } from "../types";
import { formatDuration, getJobDisplayName, timeAgo } from "../types";

interface JobRowProps {
  job: JobListItem;
  onPause?: (jobId: string) => void;
  onResume?: (jobId: string) => void;
  onCancel?: (jobId: string) => void;
}

export function JobRow({ job, onPause, onResume, onCancel }: JobRowProps) {
  const [isHovered, setIsHovered] = useState(false);

  const displayName = getJobDisplayName(job);
  const showActionButton = job.status === "running" || job.status === "paused";
  const canPause = job.status === "running" && onPause;
  const canResume = job.status === "paused" && onResume;
  const canCancel =
    (job.status === "running" || job.status === "paused") && onCancel;

  const handleAction = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (canPause) {
      onPause(job.id);
    } else if (canResume) {
      onResume(job.id);
    }
  };

  const handleCancel = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (canCancel) {
      onCancel(job.id);
    }
  };

  // Format progress percentage
  const progressPercent = Math.round(job.progress * 100);

  // Get phase and message
  const phase = job.current_phase;
  const message = job.status_message;

  // Calculate duration - prefer started_at for accuracy, fallback to created_at
  const startTime = job.started_at || job.created_at;
  const duration = startTime
    ? job.completed_at
      ? new Date(job.completed_at).getTime() - new Date(startTime).getTime()
      : Date.now() - new Date(startTime).getTime()
    : 0;

  return (
    <div
      className={clsx(
        "group relative flex items-center gap-4 border-app-line/30 border-b px-4 py-3",
        "hover:bg-app-hover/20"
      )}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {/* Icon */}
      <div className="flex-shrink-0">
        <JobStatusIndicator job={job} />
      </div>

      {/* Main info */}
      <div className="flex min-w-0 flex-1 items-center gap-6">
        {/* Job name and details */}
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <h3 className="mt-1 truncate font-medium text-ink text-sm">
              {displayName}
            </h3>
            {phase && (
              <span className="rounded-full bg-app-box px-2 py-0.5 text-ink-dull text-xs">
                {phase}
              </span>
            )}
          </div>
          {message && (
            <p className="truncate text-ink-dull text-xs">{message}</p>
          )}
        </div>

        {/* Progress / Duration column */}
        <div className="w-32 flex-shrink-0">
          {job.status === "running" || job.status === "paused" ? (
            // Show progress bar for active jobs
            <div className="flex items-center gap-2">
              <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-app-line/30">
                <div
                  className="h-full bg-accent transition-all duration-300"
                  style={{ width: `${progressPercent}%` }}
                />
              </div>
              <span className="w-8 text-right font-medium text-ink-dull text-xs">
                {progressPercent}%
              </span>
            </div>
          ) : job.status === "completed" ? (
            // Show duration for completed jobs
            <span className="text-ink-dull text-xs">
              {formatDuration(duration)}
            </span>
          ) : job.status === "queued" ? (
            // Show waiting status for queued jobs
            <span className="text-ink-dull text-xs">Waiting...</span>
          ) : (
            // Show dash for failed/cancelled jobs
            <span className="text-ink-dull text-xs">—</span>
          )}
        </div>

        {/* Completed/Started time */}
        <div className="w-24 flex-shrink-0 text-right">
          <span className="text-ink-dull text-xs">
            {job.status === "completed" && job.completed_at
              ? timeAgo(job.completed_at)
              : job.status === "running" && job.started_at
                ? timeAgo(job.started_at)
                : job.created_at
                  ? timeAgo(job.created_at)
                  : "—"}
          </span>
        </div>

        {/* Status */}
        <div className="w-20 flex-shrink-0 text-right">
          <span
            className={clsx(
              "inline-flex items-center rounded-md px-2 py-1 font-medium text-xs",
              job.status === "running" && "bg-accent/10 text-accent",
              job.status === "completed" && "bg-app-line/30 text-ink-dull",
              job.status === "failed" && "bg-red-500/10 text-red-500",
              job.status === "paused" && "bg-yellow-500/10 text-yellow-500",
              job.status === "queued" && "bg-app-line/20 text-ink-dull"
            )}
          >
            {job.status}
          </span>
        </div>
      </div>

      {/* Action buttons */}
      {isHovered && (
        <div className="flex items-center gap-1">
          {showActionButton && (canPause || canResume) && (
            <button
              className="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full bg-app-box transition-colors hover:bg-app-selected"
              onClick={handleAction}
              title={canPause ? "Pause job" : "Resume job"}
            >
              {canPause ? (
                <Pause className="text-ink" size={12} weight="fill" />
              ) : (
                <Play className="text-ink" size={12} weight="fill" />
              )}
            </button>
          )}
          {canCancel && (
            <button
              className="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full bg-app-box transition-colors hover:bg-red-500"
              onClick={handleCancel}
              title="Cancel job"
            >
              <X
                className="text-ink hover:text-white"
                size={12}
                weight="bold"
              />
            </button>
          )}
        </div>
      )}
    </div>
  );
}
