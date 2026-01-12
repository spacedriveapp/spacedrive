import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { useLibraryQuery, useLibraryMutation, useSpacedriveClient } from "../../../contexts/SpacedriveContext";
import type { JobListItem } from "../types";
import { sounds } from "@sd/assets/sounds";

// Global set to track which jobs have already played their completion sound
// This prevents multiple hook instances from playing the sound multiple times
const completedJobSounds = new Set<string>();

// Global throttle to prevent multiple sounds within 5 seconds
let lastSoundPlayedAt = 0;
const SOUND_THROTTLE_MS = 5000;

// Speed sample for historical graph
interface SpeedSample {
  timestamp: number; // Date.now()
  bytesPerSecond: number;
}

// Downsample speed history to max 100 samples
function downsampleSpeedHistory(samples: SpeedSample[]): SpeedSample[] {
  if (samples.length <= 100) return samples;

  const step = Math.ceil(samples.length / 100);
  const downsampled: SpeedSample[] = [];

  for (let i = 0; i < samples.length; i += step) {
    // Average the samples in this bucket
    const bucket = samples.slice(i, i + step);
    const avgRate =
      bucket.reduce((sum, s) => sum + s.bytesPerSecond, 0) / bucket.length;
    downsampled.push({
      timestamp: bucket[0].timestamp,
      bytesPerSecond: avgRate,
    });
  }

  return downsampled;
}

/**
 * Unified hook for job management and counting.
 * Prevents duplicate queries and subscriptions that were causing infinite loops.
 */
export function useJobs() {
  const [jobs, setJobs] = useState<JobListItem[]>([]);
  const client = useSpacedriveClient();

  // Speed history for graphing (job_id -> samples)
  const speedHistoryRef = useRef<Map<string, SpeedSample[]>>(new Map());

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
        if ("JobCompleted" in event || "JobFailed" in event || "JobCancelled" in event) {
          const jobId = event.JobCompleted?.job_id || event.JobFailed?.job_id || event.JobCancelled?.job_id;

          if (jobId) {
            // Clean up speed history for completed/failed/cancelled jobs
            speedHistoryRef.current.delete(jobId);

            if ("JobCompleted" in event && !completedJobSounds.has(jobId)) {
              completedJobSounds.add(jobId);

              // Throttle: only play sound if enough time has passed since last sound
              const now = Date.now();
              if (now - lastSoundPlayedAt >= SOUND_THROTTLE_MS) {
                lastSoundPlayedAt = now;

                // Play job-specific sound
                const jobType = event.JobCompleted?.job_type;
                if (jobType?.includes("copy") || jobType?.includes("Copy")) {
                  sounds.copy();
                } else {
                  sounds.jobDone();
                }
              }

              // Clean up old entries after 5 seconds to prevent memory leak
              setTimeout(() => completedJobSounds.delete(jobId), 5000);
            }
          }
        }
        refetchRef.current();
      } else if ("JobProgress" in event) {
        const progressData = event.JobProgress;
        if (!progressData) return;

        // Collect speed history for graphing
        if (progressData.generic_progress?.performance?.rate) {
          const jobId = progressData.job_id;
          const rate = progressData.generic_progress.performance.rate;

          const samples = speedHistoryRef.current.get(jobId) || [];
          samples.push({
            timestamp: Date.now(),
            bytesPerSecond: rate,
          });

          // Downsample if we have too many samples
          if (samples.length > 100) {
            speedHistoryRef.current.set(jobId, downsampleSpeedHistory(samples));
          } else {
            speedHistoryRef.current.set(jobId, samples);
          }
        }

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
                generic_progress: generic,
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

  // Helper to get speed history for a job (memoized to prevent re-creation)
  const getSpeedHistory = useCallback((jobId: string): SpeedSample[] => {
    return speedHistoryRef.current.get(jobId) || [];
  }, []);

  return {
    jobs,
    activeJobCount: runningCount + pausedCount,
    hasRunningJobs: runningCount > 0,
    pause,
    resume,
    cancel,
    isLoading,
    error,
    getSpeedHistory,
  };
}

// Export type for external use
export type { SpeedSample };