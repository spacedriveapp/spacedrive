import dayjs from 'dayjs';
import duration from 'dayjs/plugin/duration';
import { useEffect, useMemo } from 'react';

import { JobReport } from '../../core';
import { useForceUpdate } from '../../hooks';

dayjs.extend(duration);

// TODO: refactor this, its a mess.
export function useTotalElapsedTimeText(jobs: JobReport[] = []) {
	const forceUpdate = useForceUpdate();

	const elapsedTimeText = useMemo(() => {
		let total = 0;
		let text: string | null = '';

		const groupedJobs = jobs.reduce((acc: Record<string, JobReport[]>, job) => {
			const parentId = String(job.parent_id);
			if (!acc[parentId]) {
				acc[parentId] = [];
			}
			acc[parentId]?.push(job);
			return acc;
		}, {});

		Object.values(groupedJobs).forEach((group: JobReport[]) => {
			let groupTotal = 0;
			group.forEach((job) => {
				const start = dayjs(job.started_at);
				const end = job.completed_at ? dayjs(job.completed_at) : dayjs();

				groupTotal += end.diff(start, 'minutes');
			});

			total += groupTotal;

			const lastJob = group[group.length - 1];
			if (lastJob?.status === 'Failed' || lastJob?.status === 'Canceled') {
				text = null;
			} else {
				text = lastJob?.completed_at
					? `Took ${dayjs.duration(groupTotal, 'minutes').humanize()}`
					: null;
			}
		});

		return text;
	}, [jobs]);

	useEffect(() => {
		const allJobsCompleted = jobs.every((job) => job.completed_at);
		const isJobsQueued = jobs.some((job) => job.status === 'Queued');

		if (!allJobsCompleted || isJobsQueued) {
			const interval = setInterval(forceUpdate, 1000);
			return () => clearInterval(interval);
		}
	}, [jobs, forceUpdate]);

	return elapsedTimeText === 'Took NaN years' ? null : elapsedTimeText;
}
