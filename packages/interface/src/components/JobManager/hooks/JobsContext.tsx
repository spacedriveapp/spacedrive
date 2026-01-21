import { createContext, useContext, ReactNode } from 'react';
import { useJobs } from './useJobs';
import type { SpeedSample } from './useJobs';
import type { JobListItem } from '../types';

interface JobsContextValue {
	jobs: JobListItem[];
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

export function useJobsContext() {
	const context = useContext(JobsContext);
	if (!context) {
		throw new Error('useJobsContext must be used within JobsProvider');
	}
	return context;
}
