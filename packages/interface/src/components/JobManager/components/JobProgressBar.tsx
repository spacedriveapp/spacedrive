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
  const isPending = status === "running" && progress === 0;
  // Use gray for completed jobs, status color for running/paused
  const color = isCompleted ? "rgba(255, 255, 255, 0.2)" : JOB_STATUS_COLORS[status];
  const displayProgress = Math.min(Math.max(progress, 0), 1);

  return (
    <div
      className="w-full rounded-full overflow-hidden bg-app-line/30 mt-1.5"
      style={{ height: PROGRESS_BAR_HEIGHT }}
    >
      {isPending ? (
        <div
          className="h-full w-full animate-[barber-pole_1s_linear_infinite]"
          style={{
            backgroundImage: 'repeating-linear-gradient(45deg, rgb(0, 122, 255), rgb(0, 122, 255) 10px, rgb(80, 170, 255) 10px, rgb(80, 170, 255) 20px)',
            backgroundSize: '28px 28px'
          }}
        />
      ) : (
        <div
          className="h-full transition-all duration-300 ease-out"
          style={{
            width: `${displayProgress * 100}%`,
            backgroundColor: color,
          }}
        />
      )}
    </div>
  );
}
