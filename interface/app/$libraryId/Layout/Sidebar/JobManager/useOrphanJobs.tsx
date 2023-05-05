import { useMemo } from 'react';
import { JobReport } from '@sd/client';

export function useOrphanJobs(jobs: JobReport[], runningJobs: JobReport[]) {
	const runningJobsNoChildren = useMemo(() => {
		const singleRunningJobs = [];

		for (const job of jobs) {
			for (const runningJob of runningJobs) {
				if (
					job.parent_id !== runningJob.id &&
					job.id !== runningJob.id &&
					job.id !== job.id
				) {
					singleRunningJobs.push(runningJob);
				}
			}
		}
		return singleRunningJobs;
	}, [jobs, runningJobs]);

	return runningJobsNoChildren;
}
