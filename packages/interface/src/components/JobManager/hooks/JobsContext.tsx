import { createContext, useContext, ReactNode } from 'react';
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

const JobsContext = createContext<JobsContextValue | null>(null);

export function JobsProvider({ children }: { children: ReactNode }) {
	const jobsData = useJobs();

	return (
		<JobsContext.Provider value={jobsData}>
			{children}
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
