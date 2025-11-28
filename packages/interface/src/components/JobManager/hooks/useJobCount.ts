import { useState, useEffect, useRef } from "react";
import { useLibraryQuery, useSpacedriveClient } from "../../../context";

/**
 * Lightweight hook for job count indicator
 * Only subscribes to job state changes (not progress)
 */
export function useJobCount() {
  const client = useSpacedriveClient();
  const [activeJobCount, setActiveJobCount] = useState(0);
  const [hasRunningJobs, setHasRunningJobs] = useState(false);

  const { data } = useLibraryQuery({
    type: "jobs.list",
    input: { status: null },
  });

  // Track active jobs from query data
  useEffect(() => {
    if (data?.jobs) {
      const activeCount = data.jobs.filter(
        (job) => job.status === "running" || job.status === "paused"
      ).length;
      const hasRunning = data.jobs.some((job) => job.status === "running");

      setActiveJobCount(activeCount);
      setHasRunningJobs(hasRunning);
    }
  }, [data]);

  // Subscribe to job state changes only (not progress)
  useEffect(() => {
    if (!client) return;

    let unsubscribe: (() => void) | undefined;
    let isCancelled = false;

    const handleEvent = (event: any) => {
      // Only care about state changes, not progress
      if ("JobQueued" in event || "JobStarted" in event) {
        setActiveJobCount((prev) => prev + 1);
        if ("JobStarted" in event) {
          setHasRunningJobs(true);
        }
      } else if ("JobCompleted" in event || "JobFailed" in event || "JobCancelled" in event) {
        setActiveJobCount((prev) => Math.max(0, prev - 1));
      }
    };

    const filter = {
      event_types: ["JobQueued", "JobStarted", "JobCompleted", "JobFailed", "JobCancelled"],
    };

    client.subscribeFiltered(filter, handleEvent).then((unsub) => {
      if (isCancelled) {
        unsub();
      } else {
        unsubscribe = unsub;
      }
    });

    return () => {
      isCancelled = true;
      unsubscribe?.();
    };
  }, [client]);

  return {
    activeJobCount,
    hasRunningJobs,
  };
}
