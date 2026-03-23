import { ListBullets, CircleNotch, FunnelSimple, ArrowsOut } from "@phosphor-icons/react";
import { Popover, usePopover, TopBarButton } from "@sd/ui";
import clsx from "clsx";
import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { shouldNavigate } from "../../util/navigation";
import { motion, AnimatePresence } from "framer-motion";
import { JobList } from "./components/JobList";
import { useJobsContext } from "./hooks/JobsContext";
import { CARD_HEIGHT } from "./types";

interface JobManagerPopoverProps {
  className?: string;
}

export function JobManagerPopover({ className }: JobManagerPopoverProps) {
  const navigate = useNavigate();
  const popover = usePopover();
  const [showOnlyRunning, setShowOnlyRunning] = useState(true);

  // Unified hook for job data and badge/icon
  const { activeJobCount, hasRunningJobs, jobs, pause, resume, cancel, getSpeedHistory } = useJobsContext();

  // Reset filter to "active only" when popover opens
  useEffect(() => {
    if (popover.open) {
      setShowOnlyRunning(true);
    }
  }, [popover.open]);

  return (
    <Popover
      popover={popover}
      trigger={
        <button
          className={clsx(
            "w-full relative flex items-center gap-2 rounded-lg px-2 py-1.5 text-sm font-medium",
            "text-sidebar-inkDull cursor-default",
            className
          )}
        >
          <div className="size-4">
            {hasRunningJobs ? (
              <CircleNotch className="animate-spin" weight="bold" size={16} />
            ) : (
              <ListBullets weight="bold" size={16} />
            )}
          </div>
          <span>Jobs</span>
          {activeJobCount > 0 && (
            <span className="flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[10px] font-bold text-white bg-accent rounded-full">
              {activeJobCount}
            </span>
          )}
        </button>
      }
      side="top"
      align="start"
      sideOffset={8}
      className={clsx(
        "w-[360px] max-h-[480px] z-50",
        "!p-0 !bg-app !rounded-xl"
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-app-line">
        <h3 className="text-sm font-semibold text-ink">Job Manager</h3>

        <div className="flex items-center gap-2">
          {activeJobCount > 0 && (
            <span className="text-xs text-ink-dull">
              {activeJobCount} active
            </span>
          )}

          {/* Expand to full screen button */}
          <TopBarButton
            icon={ArrowsOut}
            onClick={(e: React.MouseEvent) => { if (!shouldNavigate(e)) return; navigate("/jobs"); }}
            title="Open full jobs screen"
          />

          {/* Filter toggle button */}
          <TopBarButton
            icon={FunnelSimple}
            active={showOnlyRunning}
            onClick={() => setShowOnlyRunning(!showOnlyRunning)}
            title={showOnlyRunning ? "Show all jobs" : "Show only active jobs"}
          />
        </div>
      </div>

      {/* Popover content with full job manager */}
      {popover.open && (
        <JobManagerPopoverContent
          jobs={jobs}
          showOnlyRunning={showOnlyRunning}
          setShowOnlyRunning={setShowOnlyRunning}
          pause={pause}
          resume={resume}
          cancel={cancel}
          getSpeedHistory={getSpeedHistory}
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
  getSpeedHistory,
}: {
  jobs: any[];
  showOnlyRunning: boolean;
  setShowOnlyRunning: (value: boolean) => void;
  pause: (jobId: string) => Promise<void>;
  resume: (jobId: string) => Promise<void>;
  cancel: (jobId: string) => Promise<void>;
  getSpeedHistory: (jobId: string) => import("./hooks/useJobs").SpeedSample[];
}) {
  const filteredJobs = showOnlyRunning
    ? jobs.filter((job) => job.status === "running" || job.status === "paused")
    : jobs;

  return (
    <motion.div
      className="overflow-y-auto no-scrollbar"
      initial={false}
      animate={{
        height:
          filteredJobs.length === 0
            ? CARD_HEIGHT + 16
            : Math.min(filteredJobs.length * (CARD_HEIGHT + 8) + 16, 400),
      }}
      transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
    >
      <JobList jobs={filteredJobs} onPause={pause} onResume={resume} onCancel={cancel} getSpeedHistory={getSpeedHistory} />
    </motion.div>
  );
}
