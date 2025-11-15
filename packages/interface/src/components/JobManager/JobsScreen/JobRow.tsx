import { Pause, Play } from "@phosphor-icons/react";
import { useState } from "react";
import clsx from "clsx";
import type { JobListItem } from "../types";
import { getJobDisplayName, formatDuration, timeAgo } from "../types";
import { JobStatusIndicator } from "../components/JobStatusIndicator";

interface JobRowProps {
  job: JobListItem;
  onPause?: (jobId: string) => void;
  onResume?: (jobId: string) => void;
}

export function JobRow({ job, onPause, onResume }: JobRowProps) {
  const [isHovered, setIsHovered] = useState(false);

  const displayName = getJobDisplayName(job);
  const showActionButton = job.status === "running" || job.status === "paused";
  const canPause = job.status === "running" && onPause;
  const canResume = job.status === "paused" && onResume;

  const handleAction = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (canPause) {
      onPause(job.id);
    } else if (canResume) {
      onResume(job.id);
    }
  };

  // Format progress percentage
  const progressPercent = Math.round(job.progress * 100);

  // Get phase and message
  const phase = job.current_phase;
  const message = job.status_message;

  // Calculate duration
  const duration = job.completed_at
    ? new Date(job.completed_at).getTime() - new Date(job.created_at).getTime()
    : Date.now() - new Date(job.created_at).getTime();

  return (
    <div
      className={clsx(
        "group relative flex items-center gap-4 px-4 py-3 border-b border-app-line/30",
        "hover:bg-app-hover/50 transition-colors cursor-pointer"
      )}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {/* Icon */}
      <div className="flex-shrink-0">
        <JobStatusIndicator job={job} />
      </div>

      {/* Main info */}
      <div className="flex-1 min-w-0 flex items-center gap-6">
        {/* Job name and details */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <h3 className="text-sm font-medium text-ink truncate">
              {displayName}
            </h3>
            {phase && (
              <span className="text-xs text-ink-dull px-2 py-0.5 rounded-full bg-app-box">
                {phase}
              </span>
            )}
          </div>
          {message && (
            <p className="text-xs text-ink-dull truncate">
              {message}
            </p>
          )}
        </div>

        {/* Progress */}
        {(job.status === "running" || job.status === "paused") && (
          <div className="flex-shrink-0 w-32">
            <div className="flex items-center gap-2">
              <div className="flex-1 h-1.5 bg-app-line/30 rounded-full overflow-hidden">
                <div
                  className="h-full bg-accent transition-all duration-300"
                  style={{ width: `${progressPercent}%` }}
                />
              </div>
              <span className="text-xs font-medium text-ink-dull w-8 text-right">
                {progressPercent}%
              </span>
            </div>
          </div>
        )}

        {/* Duration */}
        <div className="flex-shrink-0 w-20 text-right">
          <span className="text-xs text-ink-dull">
            {formatDuration(duration)}
          </span>
        </div>

        {/* Created time */}
        <div className="flex-shrink-0 w-24 text-right">
          <span className="text-xs text-ink-dull">
            {timeAgo(job.created_at)}
          </span>
        </div>

        {/* Status */}
        <div className="flex-shrink-0 w-20 text-right">
          <span
            className={clsx(
              "inline-flex items-center px-2 py-1 rounded-md text-xs font-medium",
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

      {/* Action button */}
      {showActionButton && isHovered && (canPause || canResume) && (
        <button
          onClick={handleAction}
          className="flex-shrink-0 flex items-center justify-center w-6 h-6 rounded-full bg-app-box hover:bg-app-selected transition-colors"
          title={canPause ? "Pause job" : "Resume job"}
        >
          {canPause ? (
            <Pause size={12} weight="fill" className="text-ink" />
          ) : (
            <Play size={12} weight="fill" className="text-ink" />
          )}
        </button>
      )}
    </div>
  );
}
