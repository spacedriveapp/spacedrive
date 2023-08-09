import { Folder } from '@sd/assets/icons';
import dayjs from 'dayjs';
import { useEffect, useState } from 'react';
import { Pressable, View } from 'react-native';
import {
	JobGroup as IJobGroup,
	JobProgressEvent,
	getJobNiceActionName,
	getTotalTasks,
	useLibraryMutation,
	useLibrarySubscription,
	useTotalElapsedTimeText
} from '@sd/client';
import Job from './Job';
import JobContainer from './JobContainer';

type JobGroupProps = {
	data: IJobGroup;
};

export default function JobGroup({ data: { jobs, ...data } }: JobGroupProps) {
	const [showChildJobs, setShowChildJobs] = useState(false);
	const [realtimeUpdate, setRealtimeUpdate] = useState<JobProgressEvent | null>(null);

	const pauseJob = useLibraryMutation(['jobs.pause'], {
		// onError: alert TODO:
	});
	const resumeJob = useLibraryMutation(['jobs.resume'], {
		// onError: alert TODO:
	});
	const cancelJob = useLibraryMutation(['jobs.cancel'], {
		// onError: alert TODO:
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

	return (
		<>
			{jobs?.length > 1 ? (
				<>
					<Pressable onPress={() => setShowChildJobs((v) => !v)}>
						<JobContainer
							icon={Folder}
							// TODO:
							// containerStyle
							name={getJobNiceActionName(
								data.action ?? '',
								data.status === 'Completed',
								jobs[0]
							)}
							textItems={[
								[
									{
										text: `${tasks.total} ${
											tasks.total <= 1 ? 'task' : 'tasks'
										}`
									},
									{ text: date_started },
									{ text: totalGroupTime || undefined },

									{
										text: ['Queued', 'Paused', 'Canceled', 'Failed'].includes(
											data.status
										)
											? data.status
											: undefined
									}
								],
								[
									{
										text:
											(!showChildJobs &&
												isJobsRunning &&
												realtimeUpdate?.message) ||
											undefined
									}
								]
							]}
						>
							{!showChildJobs && isJobsRunning && <>{/* TODO: ProgressBar */}</>}
						</JobContainer>
					</Pressable>
					{showChildJobs && (
						<View>
							{jobs.map((job) => (
								<Job isChild={jobs.length > 1} key={job.id} job={job} />
							))}
						</View>
					)}
				</>
			) : (
				jobs[0] && <Job job={jobs[0]} />
			)}
		</>
	);
}
