import { useState } from "react";
import { Pause, Play } from "@phosphor-icons/react";
import clsx from "clsx";
import type { JobListItem } from "../types";
import {
  CARD_HEIGHT,
  getJobDisplayName,
  getJobSubtext,
  getStatusBadge,
} from "../types";
import { JobStatusIndicator } from "./JobStatusIndicator";
import { JobProgressBar } from "./JobProgressBar";

interface JobCardProps {
  job: JobListItem;
  onPause?: (jobId: string) => void;
  onResume?: (jobId: string) => void;
}

export function JobCard({ job, onPause, onResume }: JobCardProps) {
  const [isHovered, setIsHovered] = useState(false);

  const displayName = getJobDisplayName(job);
  const subtext = getJobSubtext(job);
  const statusBadge = getStatusBadge(job);

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

  return (
    <div
      className="flex rounded-xl border border-app-line/30 bg-app-box overflow-hidden"
      style={{ height: CARD_HEIGHT }}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {/* Left icon area */}
      <JobStatusIndicator job={job} />

      {/* Divider line */}
      <div className="w-px bg-app-line/30" />

      {/* Main content area */}
      <div className="flex-1 flex flex-col gap-2 p-3.5">
        {/* Row 1: Title, badge, action button */}
        <div className="flex items-center gap-3 min-h-0">
          <span className="flex-1 truncate text-[13px] font-medium text-ink">
            {displayName}
          </span>

          <span className="flex-shrink-0 text-[11px] font-medium text-ink-dull max-w-[80px] truncate">
            {statusBadge}
          </span>

          {showActionButton && isHovered && (canPause || canResume) && (
            <button
              onClick={handleAction}
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

        {/* Row 3: Progress bar */}
        <JobProgressBar progress={job.progress} status={job.status} />
      </div>
    </div>
  );
}
