import {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import type {JobListItem, GenericProgress} from '../generated/types';
import {useLibraryMutation} from './useMutation';
import {useLibraryQuery} from './useQuery';
import {useSpacedriveClient} from './useClient';

// Speed sample for historical graph
export interface SpeedSample {
	timestamp: number; // Date.now()
	bytesPerSecond: number;
}

// Extended job with runtime progress fields from JobProgress events
export type ExtendedJobListItem = JobListItem & {
	current_phase?: string;
	current_path?: any;
	status_message?: string;
	generic_progress?: GenericProgress;
};

// Re-export GenericProgress for convenience
export type {GenericProgress};

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
			bytesPerSecond: avgRate
		});
	}

	return downsampled;
}

export interface UseJobsOptions {
	/**
	 * Callback when a job completes successfully
	 */
	onJobCompleted?: (jobId: string, jobType: string) => void;
	/**
	 * Callback when a job fails
	 */
	onJobFailed?: (jobId: string) => void;
	/**
	 * Callback when a job is cancelled
	 */
	onJobCancelled?: (jobId: string) => void;
}

export interface UseJobsReturn {
	jobs: ExtendedJobListItem[];
	activeJobCount: number;
	hasRunningJobs: boolean;
	pause: (jobId: string) => Promise<void>;
	resume: (jobId: string) => Promise<void>;
	cancel: (jobId: string) => Promise<void>;
	isLoading: boolean;
	error: any;
	getSpeedHistory: (jobId: string) => SpeedSample[];
}

/**
 * Core job management hook - shared between desktop and mobile.
 * Handles job list queries, event subscriptions, and speed history tracking.
 */
export function useJobs(options: UseJobsOptions = {}): UseJobsReturn {
	const {onJobCompleted, onJobFailed, onJobCancelled} = options;
	const [jobs, setJobs] = useState<ExtendedJobListItem[]>([]);
	const client = useSpacedriveClient();

	// Speed history for graphing (job_id -> samples)
	const speedHistoryRef = useRef<Map<string, SpeedSample[]>>(new Map());

	// Memoize input to prevent object recreation on every render
	const input = useMemo(() => ({status: null}), []);

	const {data, isLoading, error, refetch} = useLibraryQuery({
		type: 'jobs.list',
		input
	});

	const pauseMutation = useLibraryMutation('jobs.pause');
	const resumeMutation = useLibraryMutation('jobs.resume');
	const cancelMutation = useLibraryMutation('jobs.cancel');

	// Ref for stable refetch access
	const refetchRef = useRef(refetch);
	useEffect(() => {
		refetchRef.current = refetch;
	}, [refetch]);

	// Ref for stable jobs access to avoid stale closures in event handlers
	const jobsRef = useRef<ExtendedJobListItem[]>([]);
	useEffect(() => {
		jobsRef.current = jobs;
	}, [jobs]);

	useEffect(() => {
		if (data?.jobs) {
			setJobs(data.jobs as ExtendedJobListItem[]);
		}
	}, [data]);

	// Single subscription for all job events
	useEffect(() => {
		if (!client) return;

		let unsubscribe: (() => void) | undefined;
		let isCancelled = false;

		const handleEvent = (event: any) => {
			if ('JobStarted' in event) {
				refetchRef.current();
			} else if (
				'JobQueued' in event ||
				'JobCompleted' in event ||
				'JobFailed' in event ||
				'JobPaused' in event ||
				'JobResumed' in event ||
				'JobCancelled' in event
			) {
				if (
					'JobCompleted' in event ||
					'JobFailed' in event ||
					'JobCancelled' in event
				) {
					const jobId =
						event.JobCompleted?.job_id ||
						event.JobFailed?.job_id ||
						event.JobCancelled?.job_id;

					if (jobId) {
						// Set progress to 100% on completion so the UI
						// doesn't stay at 0% when a job finishes before
						// a throttled progress update is emitted.
						if ('JobCompleted' in event) {
							setJobs((prev) =>
								prev.map((job) =>
									job.id === jobId
										? { ...job, progress: 1.0 }
										: job
								)
							);
						}

						// Clean up speed history for completed/failed/cancelled jobs
						speedHistoryRef.current.delete(jobId);

						// Call callbacks
						if ('JobCompleted' in event) {
							const jobType = event.JobCompleted?.job_type || '';
							onJobCompleted?.(jobId, jobType);
						} else if ('JobFailed' in event) {
							onJobFailed?.(jobId);
						} else if ('JobCancelled' in event) {
							onJobCancelled?.(jobId);
						}
					}
				}
				refetchRef.current();
			} else if ('JobProgress' in event) {
				const progressData = event.JobProgress;
				if (!progressData) return;

				// Collect speed history for graphing
				if (progressData.generic_progress?.performance?.rate) {
					const jobId = progressData.job_id;
					const rate = progressData.generic_progress.performance.rate;

					const samples = speedHistoryRef.current.get(jobId) || [];
					samples.push({
						timestamp: Date.now(),
						bytesPerSecond: rate
					});

					// Downsample if we have too many samples
					if (samples.length > 100) {
						speedHistoryRef.current.set(
							jobId,
							downsampleSpeedHistory(samples)
						);
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
								generic_progress: generic
							})
						};
					})
				);
			}
		};

		const filter = {
			event_types: [
				'JobQueued',
				'JobStarted',
				'JobProgress',
				'JobCompleted',
				'JobFailed',
				'JobPaused',
				'JobResumed',
				'JobCancelled'
			]
		};

		client.subscribeFiltered(filter, handleEvent).then((unsub: () => void) => {
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
	}, [client, onJobCompleted, onJobFailed, onJobCancelled]);

	const pause = async (jobId: string) => {
		try {
			const result = await pauseMutation.mutateAsync({job_id: jobId});
			if (!result.success) {
				console.error('Failed to pause job:', jobId);
			}
		} catch (error) {
			console.error('Failed to pause job:', error);
		}
	};

	const resume = async (jobId: string) => {
		try {
			const result = await resumeMutation.mutateAsync({job_id: jobId});
			if (!result.success) {
				console.error('Failed to resume job:', jobId);
			}
		} catch (error) {
			console.error('Failed to resume job:', error);
		}
	};

	const cancel = async (jobId: string) => {
		try {
			const result = await cancelMutation.mutateAsync({job_id: jobId});
			if (!result.success) {
				console.error('Failed to cancel job:', jobId);
			}
		} catch (error) {
			console.error('Failed to cancel job:', error);
		}
	};

	const runningCount = jobs.filter((j) => j.status === 'running').length;
	const pausedCount = jobs.filter((j) => j.status === 'paused').length;

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
		getSpeedHistory
	};
}
