import { CircleNotch } from "@phosphor-icons/react";
import type { JobListItem } from "@sd/ts-client";

interface DeviceJobActivityProps {
  jobs: JobListItem[];
}

export function DeviceJobActivity({ jobs }: DeviceJobActivityProps) {
  const activeJobs = jobs.filter(
    (j) => j.status === "running" || j.status === "paused"
  );

  if (activeJobs.length === 0) {
    return null;
  }

  const firstJob = activeJobs[0];
  const remainingCount = activeJobs.length - 1;

  return (
    <div className="ml-auto flex items-center gap-2">
      <div className="flex items-center gap-1.5 rounded-md border border-accent/20 bg-accent/10 px-2 py-1 text-accent text-xs">
        <CircleNotch className="animate-spin" size={12} weight="bold" />
        <span className="font-medium">{firstJob.name}</span>
        <span className="text-accent/70">
          {Math.round(firstJob.progress * 100)}%
        </span>
      </div>
      {remainingCount > 0 && (
        <span className="text-ink-dull text-xs">+{remainingCount} more</span>
      )}
    </div>
  );
}
