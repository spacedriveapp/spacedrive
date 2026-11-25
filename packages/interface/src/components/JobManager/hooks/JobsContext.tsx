import { createContext, useContext, useMemo, ReactNode } from 'react';
import { useJobs } from './useJobsDesktop';
import type { SpeedSample, ExtendedJobListItem } from '@sd/ts-client';

interface JobsContextValue {
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

interface JobCounts {
	activeJobCount: number;
	hasRunningJobs: boolean;
}

const JobsContext = createContext<JobsContextValue | null>(null);
const JobCountsContext = createContext<JobCounts | null>(null);

export function JobsProvider({ children }: { children: ReactNode }) {
	const jobsData = useJobs();
	const { activeJobCount, hasRunningJobs } = jobsData;
	// Every progress tick replaces the job list, so consumers that only show
	// counts read them from their own context and skip those ticks.
	const counts = useMemo(
		() => ({ activeJobCount, hasRunningJobs }),
		[activeJobCount, hasRunningJobs]
	);

	return (
		<JobsContext.Provider value={jobsData}>
			<JobCountsContext.Provider value={counts}>
				{children}
			</JobCountsContext.Provider>
		</JobsContext.Provider>
	);
}

/** The jobs list, or null in a window without a JobsProvider. */
export function useOptionalJobsContext() {
	return useContext(JobsContext);
}

export function useJobsContext() {
	const context = useContext(JobsContext);
	if (!context) {
		throw new Error('useJobsContext must be used within JobsProvider');
	}
	return context;
}

/** Active and running job counts, which change far less often than the list. */
export function useJobCounts() {
	const context = useContext(JobCountsContext);
	if (!context) {
		throw new Error('useJobCounts must be used within JobsProvider');
	}
	return context;
}
