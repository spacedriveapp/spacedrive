import { useState, useEffect } from "react";
import { useLibraryQuery, useLibraryMutation } from "../../../context";
import { useEvent } from "../../../hooks/useEvent";
import type { JobListItem } from "../types";

export function useJobManager() {
  const [jobs, setJobs] = useState<JobListItem[]>([]);

  const { data, isLoading, error, refetch } = useLibraryQuery({
    type: "jobs.list",
    input: { status: null },
  });

  const pauseMutation = useLibraryMutation("jobs.pause");
  const resumeMutation = useLibraryMutation("jobs.resume");

  useEffect(() => {
    if (data?.jobs) {
      setJobs(data.jobs);
    }
  }, [data]);

  useEvent("JobQueued", () => {
    refetch();
  });

  useEvent("JobStarted", () => {
    refetch();
  });

  useEvent("JobProgress", (event: any) => {
    const progressData = event.JobProgress;
    if (!progressData) return;

    setJobs((prev) =>
      prev.map((job) => {
        if (job.id !== progressData.job_id) return job;

        // Extract rich metadata from generic_progress
        const generic = progressData.generic_progress;

        return {
          ...job,
          progress: progressData.progress,
          // Store additional metadata for better UI
          ...(generic && {
            current_phase: generic.phase,
            current_path: generic.current_path,
            status_message: generic.message,
          }),
        };
      }),
    );
  });

  useEvent("JobCompleted", () => {
    refetch();
  });

  useEvent("JobFailed", () => {
    refetch();
  });

  useEvent("JobPaused", () => {
    refetch();
  });

  useEvent("JobResumed", () => {
    refetch();
  });

  useEvent("JobCancelled", () => {
    refetch();
  });

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
