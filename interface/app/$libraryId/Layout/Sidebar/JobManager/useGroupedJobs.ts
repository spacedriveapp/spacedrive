import { useMemo } from 'react';
import { JobReport } from '@sd/client';

export interface IJobGroup extends JobReport {
	childJobs: JobReport[];
	runningJobs: JobReport[];
}

export function useGroupedJobs(jobs: JobReport[] = [], runningJobs: JobReport[] = []) {
	return useMemo(() => {
		return jobs.reduce((arr, job) => {
			const childJobs = jobs
				.filter((j) => j.parent_id === job.id || j.id === job.id)
				// sort by started_at, a string date that is possibly null
				.sort((a, b) => {
					if (!a.started_at && !b.started_at) {
						return 0;
					}

					if (!a.started_at) {
						// a is null
						return 1;
					}

					return a.started_at.localeCompare(b.started_at || '');
				});

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
