import dayjs from 'dayjs';
import { useEffect, useMemo } from 'react';
import { JobReport } from '@sd/client';
import { useForceUpdate } from '~/util';

export function useJobTimeText(job: JobReport): string | null {
	const forceUpdate = useForceUpdate();

	const elapsedTimeText = useMemo(() => {
		let newText: string;
		if (job.status === 'Running') {
			newText = `Elapsed in ${dayjs(job.started_at).fromNow(true)}`;
		} else if (job.completed_at) {
			newText = `Took ${dayjs(job.started_at).from(job.completed_at, true)}`;
		} else {
			newText = `Took ${dayjs(job.started_at).fromNow(true)}`;
		}
		return newText;
	}, [job]);

	useEffect(() => {
		if (job.status === 'Running') {
			const interval = setInterval(forceUpdate, 1000);
			return () => clearInterval(interval);
		}
	}, [job.status, forceUpdate]);

	return elapsedTimeText === 'Took NaN years' ? null : elapsedTimeText;
}
