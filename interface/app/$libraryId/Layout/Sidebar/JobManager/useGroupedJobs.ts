import { useMemo } from 'react';
import { JobReport } from '@sd/client';

export interface IJobGroup extends JobReport {
	childJobs: JobReport[];
	runningJobs: JobReport[];
}

export function useGroupedJobs(jobs: JobReport[] = [], runningJobs: JobReport[] = []) {
	return useMemo(() => {
		return jobs.reduce((arr, job) => {
			const childJobs = jobs.filter((j) => j.parent_id === job.id);

			if (!jobs.some((j) => j.id === job.parent_id)) {
				arr.push({
					...job,
					childJobs,
					runningJobs: runningJobs.filter((j) => j.parent_id === job.id)
				});
			}

			return arr;
		}, [] as IJobGroup[]);
	}, [jobs, runningJobs]);
}
