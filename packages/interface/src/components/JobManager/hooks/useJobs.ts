import { useState, useEffect, useRef, useMemo } from "react";
import { useLibraryQuery, useLibraryMutation, useSpacedriveClient } from "../../../context";
import type { JobListItem } from "../types";
import { sounds } from "@sd/assets/sounds";

// Global set to track which jobs have already played their completion sound
// This prevents multiple hook instances from playing the sound multiple times
const completedJobSounds = new Set<string>();

/**
 * Unified hook for job management and counting.
 * Prevents duplicate queries and subscriptions that were causing infinite loops.
 */
export function useJobs() {
  const [jobs, setJobs] = useState<JobListItem[]>([]);
  const client = useSpacedriveClient();

  // Memoize input to prevent object recreation on every render
  const input = useMemo(() => ({ status: null }), []);

  const { data, isLoading, error, refetch } = useLibraryQuery({
    type: "jobs.list",
    input,
  });

  const pauseMutation = useLibraryMutation("jobs.pause");
  const resumeMutation = useLibraryMutation("jobs.resume");
  const cancelMutation = useLibraryMutation("jobs.cancel");

  // Ref for stable refetch access
  const refetchRef = useRef(refetch);
  useEffect(() => {
    refetchRef.current = refetch;
  }, [refetch]);

  useEffect(() => {
    if (data?.jobs) {
      setJobs(data.jobs);
    }
  }, [data]);

  // Single subscription for all job events
  useEffect(() => {
    if (!client) return;

    let unsubscribe: (() => void) | undefined;
    let isCancelled = false;

    const handleEvent = (event: any) => {
      if ("JobQueued" in event || "JobStarted" in event || "JobCompleted" in event ||
          "JobFailed" in event || "JobPaused" in event || "JobResumed" in event ||
          "JobCancelled" in event) {
        if ("JobCompleted" in event) {
          const jobId = event.JobCompleted?.job_id;
          const jobType = event.JobCompleted?.job_type;
          if (jobId && !completedJobSounds.has(jobId)) {
            completedJobSounds.add(jobId);

            // Play job-specific sound
            if (jobType?.includes("copy") || jobType?.includes("Copy")) {
              sounds.copy();
            } else {
              sounds.jobDone();
            }

            // Clean up old entries after 5 seconds to prevent memory leak
            setTimeout(() => completedJobSounds.delete(jobId), 5000);
          }
        }
        refetchRef.current();
      } else if ("JobProgress" in event) {
        const progressData = event.JobProgress;
        if (!progressData) return;

        setJobs((prev) =>
          prev.map((job) => {
            if (job.id !== progressData.job_id) return job;

            const generic = progressData.generic_progress;

            return {
              ...job,
              progress: progressData.progress,
              ...(generic && {
                current_phase: generic.phase,
                current_path: generic.current_path,
                status_message: generic.message,
              }),
            };
          }),
        );
      }
    };

    const filter = {
      event_types: ["JobQueued", "JobStarted", "JobProgress", "JobCompleted", "JobFailed", "JobPaused", "JobResumed", "JobCancelled"],
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

  const pause = async (jobId: string) => {
    try {
      const result = await pauseMutation.mutateAsync({ job_id: jobId });
      if (!result.success) {
        console.error("Failed to pause job:", jobId);
      }
    } catch (error) {
      console.error("Failed to pause job:", error);
    }
  };

  const resume = async (jobId: string) => {
    try {
      const result = await resumeMutation.mutateAsync({ job_id: jobId });
      if (!result.success) {
        console.error("Failed to resume job:", jobId);
      }
    } catch (error) {
      console.error("Failed to resume job:", error);
    }
  };

  const cancel = async (jobId: string) => {
    try {
      const result = await cancelMutation.mutateAsync({ job_id: jobId });
      if (!result.success) {
        console.error("Failed to cancel job:", jobId);
      }
    } catch (error) {
      console.error("Failed to cancel job:", error);
    }
  };

  const runningCount = jobs.filter((j) => j.status === "running").length;
  const pausedCount = jobs.filter((j) => j.status === "paused").length;

  return {
    jobs,
    activeJobCount: runningCount + pausedCount,
    hasRunningJobs: runningCount > 0,
    pause,
    resume,
    cancel,
    isLoading,
    error,
  };
}
