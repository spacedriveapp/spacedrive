import { Folder } from '@sd/assets/icons';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { Pause, Play, Stop } from 'phosphor-react';
import { Fragment, useEffect, useState } from 'react';
import {
	JobGroup as IJobGroup,
	JobProgressEvent,
	getJobNiceActionName,
	getTotalTasks,
	useLibraryMutation,
	useLibrarySubscription,
	useTotalElapsedTimeText
} from '@sd/client';
import { Button, ProgressBar, Tooltip } from '@sd/ui';
import Job from './Job';
import JobContainer from './JobContainer';

interface JobGroupProps {
	data: IJobGroup;
}

function JobGroup({ data: { jobs, ...data } }: JobGroupProps) {
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

	return (
		<ul className="relative overflow-hidden">
			<div className="row absolute right-3 top-3 z-50 flex space-x-1">
				{/* Resume */}
				{(data.status === 'Queued' || data.status === 'Paused' || isJobPaused) && (
					<Button
						className="cursor-pointer"
						onClick={() => resumeJob.mutate(data.id)}
						size="icon"
						variant="outline"
					>
						<Tooltip label="Resume">
							<Play className="h-4 w-4 cursor-pointer" />
						</Tooltip>
					</Button>
				)}
				{/* Pause/Stop */}
				{isJobsRunning && (
					<Fragment>
						<Tooltip label="Pause">
							<Button
								className="cursor-pointer"
								onClick={() => {
									pauseJob.mutate(data.id);
								}}
								size="icon"
								variant="outline"
							>
								<Pause className="h-4 w-4 cursor-pointer" />
							</Button>
						</Tooltip>
						<Tooltip label="Stop">
							<Button
								className="cursor-pointer"
								onClick={() => {
									cancelJob.mutate(data.id);
								}}
								size="icon"
								variant="outline"
							>
								<Stop className="h-4 w-4 cursor-pointer" />
							</Button>
						</Tooltip>
					</Fragment>
				)}
				{/* Remove */}
				{/* TODO: Implement this */}
				{/* {!isJobsRunning && (
					<Button className="cursor-pointer" size="icon" variant="outline">
						<Tooltip label="Remove">
							<DotsThreeVertical className="h-4 w-4 cursor-pointer" />
						</Tooltip>
					</Button>
				)} */}
			</div>
			{jobs?.length > 1 ? (
				<>
					<JobContainer
						onClick={() => setShowChildJobs((v) => !v)}
						className={clsx(
							'pb-2 hover:bg-app-selected/10',
							showChildJobs && 'border-none bg-app-darkBox pb-1 hover:!bg-app-darkBox'
						)}
						icon={Folder}
						name={getJobNiceActionName(
							data.action ?? '',
							data.status === 'Completed',
							jobs[0]
						)}
						textItems={[
							[
								{ text: `${tasks.total} ${tasks.total <= 1 ? 'task' : 'tasks'}` },
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
						{!showChildJobs && isJobsRunning && (
							<div className="my-1 ml-1.5 w-full">
								<ProgressBar
									pending={tasks.completed === 0}
									value={tasks.completed}
									total={tasks.total}
								/>
							</div>
						)}
					</JobContainer>
					{showChildJobs && (
						<div>
							{jobs.map((job) => (
								<Job isChild={jobs.length > 1} key={job.id} job={job} />
							))}
						</div>
					)}
				</>
			) : (
				<>{jobs[0] && <Job job={jobs[0]} />}</>
			)}
		</ul>
	);
}

export default JobGroup;
