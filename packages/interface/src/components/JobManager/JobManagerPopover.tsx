import {
  ArrowsOut,
  CircleNotch,
  FunnelSimple,
  ListBullets,
} from "@phosphor-icons/react";
import { Popover, TopBarButton, usePopover } from "@sd/ui";
import clsx from "clsx";
import { motion } from "framer-motion";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { JobList } from "./components/JobList";
import { useJobs } from "./hooks/useJobs";
import { CARD_HEIGHT } from "./types";

interface JobManagerPopoverProps {
  className?: string;
}

export function JobManagerPopover({ className }: JobManagerPopoverProps) {
  const navigate = useNavigate();
  const popover = usePopover();
  const [showOnlyRunning, setShowOnlyRunning] = useState(true);

  // Unified hook for job data and badge/icon
  const { activeJobCount, hasRunningJobs, jobs, pause, resume, cancel } =
    useJobs();

  // Reset filter to "active only" when popover opens
  useEffect(() => {
    if (popover.open) {
      setShowOnlyRunning(true);
    }
  }, [popover.open]);

  return (
    <Popover
      align="start"
      className={clsx(
        "z-50 max-h-[480px] w-[360px]",
        "!p-0 !bg-app !rounded-xl"
      )}
      popover={popover}
      side="top"
      sideOffset={8}
      trigger={
        <button
          className={clsx(
            "relative flex w-full items-center gap-2 rounded-lg px-2 py-1.5 font-medium text-sm",
            "cursor-default text-sidebar-inkDull",
            className
          )}
        >
          <div className="size-4">
            {hasRunningJobs ? (
              <CircleNotch className="animate-spin" size={16} weight="bold" />
            ) : (
              <ListBullets size={16} weight="bold" />
            )}
          </div>
          <span>Jobs</span>
          {activeJobCount > 0 && (
            <span className="flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-accent px-1 font-bold text-[10px] text-white">
              {activeJobCount}
            </span>
          )}
        </button>
      }
    >
      {/* Header */}
      <div className="flex items-center justify-between border-app-line border-b px-4 py-3">
        <h3 className="font-semibold text-ink text-sm">Job Manager</h3>

        <div className="flex items-center gap-2">
          {activeJobCount > 0 && (
            <span className="text-ink-dull text-xs">
              {activeJobCount} active
            </span>
          )}

          {/* Expand to full screen button */}
          <TopBarButton
            icon={ArrowsOut}
            onClick={() => navigate("/jobs")}
            title="Open full jobs screen"
          />

          {/* Filter toggle button */}
          <TopBarButton
            active={showOnlyRunning}
            icon={FunnelSimple}
            onClick={() => setShowOnlyRunning(!showOnlyRunning)}
            title={showOnlyRunning ? "Show all jobs" : "Show only active jobs"}
          />
        </div>
      </div>

      {/* Popover content with full job manager */}
      {popover.open && (
        <JobManagerPopoverContent
          cancel={cancel}
          jobs={jobs}
          pause={pause}
          resume={resume}
          setShowOnlyRunning={setShowOnlyRunning}
          showOnlyRunning={showOnlyRunning}
        />
      )}
    </Popover>
  );
}

function JobManagerPopoverContent({
  jobs,
  showOnlyRunning,
  setShowOnlyRunning,
  pause,
  resume,
  cancel,
}: {
  jobs: any[];
  showOnlyRunning: boolean;
  setShowOnlyRunning: (value: boolean) => void;
  pause: (jobId: string) => Promise<void>;
  resume: (jobId: string) => Promise<void>;
  cancel: (jobId: string) => Promise<void>;
}) {
  const filteredJobs = showOnlyRunning
    ? jobs.filter((job) => job.status === "running" || job.status === "paused")
    : jobs;

  return (
    <motion.div
      animate={{
        height:
          filteredJobs.length === 0
            ? CARD_HEIGHT + 16
            : Math.min(filteredJobs.length * (CARD_HEIGHT + 8) + 16, 400),
      }}
      className="no-scrollbar overflow-y-auto"
      initial={false}
      transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
    >
      <JobList
        jobs={filteredJobs}
        onCancel={cancel}
        onPause={pause}
        onResume={resume}
      />
    </motion.div>
  );
}
