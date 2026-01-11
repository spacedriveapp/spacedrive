import { useState } from "react";
import { Pause, Play, X, CaretDown } from "@phosphor-icons/react";
import { motion, AnimatePresence } from "framer-motion";
import clsx from "clsx";
import type { JobListItem } from "../types";
import type { SpeedSample } from "../hooks/useJobs";
import {
  CARD_HEIGHT,
  getJobDisplayName,
  getJobSubtext,
  getStatusBadge,
} from "../types";
import { JobStatusIndicator } from "./JobStatusIndicator";
import { JobProgressBar } from "./JobProgressBar";
import { CopyJobDetails } from "./CopyJobDetails";

interface JobCardProps {
  job: JobListItem;
  onPause?: (jobId: string) => void;
  onResume?: (jobId: string) => void;
  onCancel?: (jobId: string) => void;
  getSpeedHistory: (jobId: string) => SpeedSample[];
}

export function JobCard({ job, onPause, onResume, onCancel, getSpeedHistory }: JobCardProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);

  const displayName = getJobDisplayName(job);
  const subtext = getJobSubtext(job);
  const statusBadge = getStatusBadge(job);

  const showActionButton = job.status === "running" || job.status === "paused";
  const canPause = job.status === "running" && onPause;
  const canResume = job.status === "paused" && onResume;
  const canCancel = (job.status === "running" || job.status === "paused") && onCancel;

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

  const toggleExpanded = () => {
    setIsExpanded(!isExpanded);
  };

  const isCopyJob = job.name === "file_copy";

  return (
    <motion.div
      layout
      className="rounded-xl border border-app-line/30 bg-app-box overflow-hidden"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <div
        className="flex cursor-pointer"
        style={{ height: CARD_HEIGHT }}
        onClick={toggleExpanded}
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

          {/* Expansion caret */}
          {isCopyJob && (
            <motion.div
              animate={{ rotate: isExpanded ? 180 : 0 }}
              transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
              className="flex-shrink-0"
            >
              <CaretDown size={12} weight="bold" className="text-ink-dull" />
            </motion.div>
          )}

          {isHovered && (
            <div className="flex items-center gap-1">
              {showActionButton && (canPause || canResume) && (
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
              {canCancel && (
                <button
                  onClick={handleCancel}
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

        {/* Row 3: Progress bar */}
        <JobProgressBar progress={job.progress} status={job.status} />
      </div>
      </div>

      {/* Expanded details section */}
      <AnimatePresence>
        {isExpanded && isCopyJob && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
            className="overflow-hidden"
          >
            <div className="border-t border-app-line/30">
              <CopyJobDetails job={job} speedHistory={getSpeedHistory(job.id)} />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
