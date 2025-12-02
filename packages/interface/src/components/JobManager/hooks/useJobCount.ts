import { useEffect } from "react";
import { useLibraryQuery, useSpacedriveClient } from "../../../context";

/**
 * Lightweight hook for job count indicator.
 * Uses jobs.active query which only returns in-memory active jobs (not thousands from DB).
 * Events trigger a refetch rather than incrementing/decrementing counts manually.
 */
export function useJobCount() {
  const client = useSpacedriveClient();

  const { data, refetch } = useLibraryQuery({
    type: "jobs.list",
    input: { status: null },
  });

  // Subscribe to job state changes and refetch when they occur
  useEffect(() => {
    if (!client) return;

    let unsubscribe: (() => void) | undefined;
    let isCancelled = false;

    const filter = {
      event_types: [
        "JobQueued",
        "JobStarted",
        "JobCompleted",
        "JobFailed",
        "JobCancelled",
        "JobPaused",
        "JobResumed",
      ],
    };

    client.subscribeFiltered(filter, () => refetch()).then((unsub) => {
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
  }, [client, refetch]);

  const jobs = data?.jobs ?? [];
  const runningCount = jobs.filter(j => j.status === "running").length;
  const pausedCount = jobs.filter(j => j.status === "paused").length;

  return {
    activeJobCount: runningCount + pausedCount,
    hasRunningJobs: runningCount > 0,
  };
}
