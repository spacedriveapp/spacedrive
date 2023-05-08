import { useMemo } from 'react';
import { JobReport } from '@sd/client';

// This could be done better, as the memo is kinda redundant. Large lists of jobs will be slow.
export function useFilteredJobs(jobs: JobReport[] = [], runningJobs: JobReport[] = []) {
	return useMemo(() => {
		const individualJobs = jobs.filter((job) => job.action === null);
		const runningIndividualJobs = runningJobs.filter((job) => job.action === null);
		const jobsWithActions = jobs.filter((job) => job.action !== null);
		const runningJobsWithActions = runningJobs.filter((job) => job.action !== null);

		return {
			individualJobs,
			runningIndividualJobs,
			jobsWithActions,
			runningJobsWithActions
		};
	}, [jobs, runningJobs]);
}
