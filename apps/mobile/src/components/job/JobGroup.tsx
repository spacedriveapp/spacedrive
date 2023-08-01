import { JobGroup, JobProgressEvent, getTotalTasks, useLibraryMutation, useLibrarySubscription, useTotalElapsedTimeText } from "@sd/client";
import dayjs from "dayjs";
import { useEffect, useState } from "react";

type JobGroupProps = {
	data: JobGroup;
};

function JobGroup({data: {jobs, ...data}}: JobGroupProps) {
  const [showChildJobs, setShowChildJobs] = useState(false);
	const [realtimeUpdate, setRealtimeUpdate] = useState<JobProgressEvent | null>(null);

	const pauseJob = useLibraryMutation(['jobs.pause'], {
		onError: alert
	});
	const resumeJob = useLibraryMutation(['jobs.resume'], {
		onError: alert
	});
	const cancelJob = useLibraryMutation(['jobs.cancel'], {
		onError: alert
	});

	const isJobsRunning = jobs.some((job) => job.status === 'Running');
	const isJobPaused = jobs.some((job) => job.status === 'Paused');
	const activeJobId = jobs.find((job) => job.status === 'Running')?.id;

	useLibrarySubscription(['jobs.progress', activeJobId as string], {
		onData: setRealtimeUpdate,
		enabled: !!activeJobId || !showChildJobs
	});

	useEffect(() => {
		if (data.status !== 'Running') {
			setRealtimeUpdate(null);
		}
	}, [data.status]);

  const tasks = getTotalTasks(jobs);
	const totalGroupTime = useTotalElapsedTimeText(jobs);

	if (!jobs.length) return <></>;

	let date_started = dayjs(jobs[0]?.created_at).fromNow();
	date_started = date_started.charAt(0).toUpperCase() + date_started.slice(1);
  
	return <></>;
}
