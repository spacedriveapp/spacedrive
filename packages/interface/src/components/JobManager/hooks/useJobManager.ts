import { useState, useEffect, useRef } from "react";
import { useLibraryQuery, useLibraryMutation, useSpacedriveClient } from "../../../context";
import type { JobListItem } from "../types";

export function useJobManager() {
  const [jobs, setJobs] = useState<JobListItem[]>([]);
  const client = useSpacedriveClient();

  const { data, isLoading, error, refetch } = useLibraryQuery({
    type: "jobs.list",
    input: { status: null },
  });

  const pauseMutation = useLibraryMutation("jobs.pause");
  const resumeMutation = useLibraryMutation("jobs.resume");

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

  // Subscribe to job events using filtered subscription
  useEffect(() => {
    if (!client) return;

    let unsubscribe: (() => void) | undefined;
    let isCancelled = false;

    const handleEvent = (event: any) => {
      if ("JobQueued" in event || "JobStarted" in event || "JobCompleted" in event ||
          "JobFailed" in event || "JobPaused" in event || "JobResumed" in event ||
          "JobCancelled" in event) {
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

    // Subscribe to job events only
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
    await pauseMutation.mutateAsync({ job_id: jobId });
  };

  const resume = async (jobId: string) => {
    await resumeMutation.mutateAsync({ job_id: jobId });
  };

  const activeJobCount = jobs.filter(
    (job) => job.status === "running" || job.status === "paused",
  ).length;

  return {
    jobs,
    activeJobCount,
    pause,
    resume,
    isLoading,
    error,
  };
}
