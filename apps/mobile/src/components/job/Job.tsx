import { Question } from 'phosphor-react-native';
import { memo, useEffect, useState } from 'react';
import { JobProgressEvent, JobReport, useJobInfo, useLibrarySubscription } from '@sd/client';

type JobProps = {
	job: JobReport;
};

function Job({ job }: JobProps) {
	const [realtimeUpdate, setRealtimeUpdate] = useState<JobProgressEvent | null>(null);

	useLibrarySubscription(['jobs.progress', job.id], {
		onData: setRealtimeUpdate
	});

	const niceData = useJobInfo(job, realtimeUpdate)[job.name] || {
		name: job.name,
		icon: Question,
		textItems: [[{ text: job.status.replace(/([A-Z])/g, ' $1').trim() }]]
	};
	const isRunning = job.status === 'Running';
	const isPaused = job.status === 'Paused';

	const task_count = realtimeUpdate?.task_count || job.task_count;
	const completed_task_count = realtimeUpdate?.completed_task_count || job.completed_task_count;

	// clear stale realtime state when job is done
	useEffect(() => {
		if (isRunning) setRealtimeUpdate(null);
	}, [isRunning]);

	return <></>;
}

export default memo(Job);
