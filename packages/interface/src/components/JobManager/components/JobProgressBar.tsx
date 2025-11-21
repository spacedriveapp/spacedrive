import type { JobStatus } from "@sd/ts-client";
import { JOB_STATUS_COLORS, PROGRESS_BAR_HEIGHT } from "../types";

interface JobProgressBarProps {
  progress: number;
  status: JobStatus;
}

export function JobProgressBar({ progress, status }: JobProgressBarProps) {
  // Only show progress bar for running, paused jobs, or completed jobs with some visual feedback
  const showProgress = status === "running" || status === "paused" || status === "completed";

  if (!showProgress) {
    return <div style={{ height: PROGRESS_BAR_HEIGHT }} />;
  }

  const isCompleted = status === "completed";
  // Use gray for completed jobs, status color for running/paused
  const color = isCompleted ? "rgba(255, 255, 255, 0.2)" : JOB_STATUS_COLORS[status];
  const displayProgress = Math.min(Math.max(progress, 0), 1);

  return (
    <div
      className="w-full rounded-full overflow-hidden bg-app-line/30 mt-1.5"
      style={{ height: PROGRESS_BAR_HEIGHT }}
    >
      <div
        className="h-full transition-all duration-300 ease-out"
        style={{
          width: `${displayProgress * 100}%`,
          backgroundColor: color,
        }}
      />
    </div>
  );
}
