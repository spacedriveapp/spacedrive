import { Pause, Play, X } from "@phosphor-icons/react";
import { useState } from "react";
import type { JobListItem } from "../types";
import {
  CARD_HEIGHT,
  getJobDisplayName,
  getJobSubtext,
  getStatusBadge,
} from "../types";
import { JobProgressBar } from "./JobProgressBar";
import { JobStatusIndicator } from "./JobStatusIndicator";

interface JobCardProps {
  job: JobListItem;
  onPause?: (jobId: string) => void;
  onResume?: (jobId: string) => void;
  onCancel?: (jobId: string) => void;
}

export function JobCard({ job, onPause, onResume, onCancel }: JobCardProps) {
  const [isHovered, setIsHovered] = useState(false);

  const displayName = getJobDisplayName(job);
  const subtext = getJobSubtext(job);
  const statusBadge = getStatusBadge(job);

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

  return (
    <div
      className="flex overflow-hidden rounded-xl border border-app-line/30 bg-app-box"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      style={{ height: CARD_HEIGHT }}
    >
      {/* Left icon area */}
      <JobStatusIndicator job={job} />

      {/* Divider line */}
      <div className="w-px bg-app-line/30" />

      {/* Main content area */}
      <div className="flex flex-1 flex-col gap-2 p-3.5">
        {/* Row 1: Title, badge, action button */}
        <div className="flex min-h-0 items-center gap-3">
          <span className="flex-1 truncate font-medium text-[13px] text-ink">
            {displayName}
          </span>

          <span className="max-w-[80px] flex-shrink-0 truncate font-medium text-[11px] text-ink-dull">
            {statusBadge}
          </span>

          {isHovered && (
            <div className="flex items-center gap-1">
              {showActionButton && (canPause || canResume) && (
                <button
                  className="flex h-4 w-4 flex-shrink-0 items-center justify-center rounded-full bg-app-hover transition-colors hover:bg-app-selected"
                  onClick={handleAction}
                  title={canPause ? "Pause job" : "Resume job"}
                >
                  {canPause ? (
                    <Pause className="text-ink" size={10} weight="fill" />
                  ) : (
                    <Play className="text-ink" size={10} weight="fill" />
                  )}
                </button>
              )}
              {canCancel && (
                <button
                  className="flex h-4 w-4 flex-shrink-0 items-center justify-center rounded-full bg-app-hover transition-colors hover:bg-red-500"
                  onClick={handleCancel}
                  title="Cancel job"
                >
                  <X
                    className="text-ink hover:text-white"
                    size={10}
                    weight="bold"
                  />
                </button>
              )}
            </div>
          )}
        </div>

        {/* Row 2: Subtext */}
        <div className="min-h-0">
          <span
            className="block max-w-[200px] truncate text-[10px] text-ink-dull"
            style={{ opacity: 0.7 }}
          >
            {subtext}
          </span>
        </div>

        {/* Row 3: Progress bar */}
        <JobProgressBar progress={job.progress} status={job.status} />
      </div>
    </div>
  );
}
