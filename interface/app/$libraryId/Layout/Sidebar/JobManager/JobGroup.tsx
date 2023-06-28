/* eslint-disable no-case-declarations */
import { Folder } from '@sd/assets/icons';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { DotsThreeVertical, Pause, Play, Stop } from 'phosphor-react';
import { Fragment, useEffect, useState } from 'react';
import {
	JobGroup as IJobGroup,
	JobProgressEvent,
	JobReport,
	useLibraryMutation,
	useLibrarySubscription
} from '@sd/client';
import { Button, ProgressBar, Tooltip } from '@sd/ui';
import Job from './Job';
import JobContainer from './JobContainer';
import { useTotalElapsedTimeText } from './useGroupJobTimeText';

interface JobGroupProps {
	data: IJobGroup;
	clearJob: (arg: string) => void;
}

function JobGroup({ data: { jobs, ...data }, clearJob }: JobGroupProps) {
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

	const tasks = totalTasks(jobs);
	const totalGroupTime = useTotalElapsedTimeText(jobs);

	if (!jobs.length) return <></>;

	let date_started = dayjs(jobs[0]?.created_at).fromNow();
	date_started = date_started.charAt(0).toUpperCase() + date_started.slice(1);

	return (
		<ul className="relative overflow-hidden">
			<div className="row absolute right-3 top-3 z-50 flex space-x-1">
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

				{/* TODO: FIX THIS, why is this not working? */}

				{!isJobsRunning && (
					<Button
						className="hidden cursor-pointer"
						onClick={() => clearJob?.(data.id as string)}
						size="icon"
						variant="outline"
					>
						<Tooltip label="Remove">
							<DotsThreeVertical className="h-4 w-4 cursor-pointer" />
						</Tooltip>
					</Button>
				)}
			</div>
			{jobs?.length > 1 ? (
				<>
					<JobContainer
						onClick={() => setShowChildJobs((v) => !v)}
						className={clsx(
							'pb-2 hover:bg-app-selected/10',
							showChildJobs && 'border-none bg-app-darkBox pb-1 hover:!bg-app-darkBox'
						)}
						iconImg={Folder}
						name={niceActionName(
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
						<div className="">
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

function totalTasks(jobs: JobReport[]) {
	const tasks = { completed: 0, total: 0, timeOfLastFinishedJob: '' };

	jobs?.forEach(({ task_count, status, completed_at, completed_task_count }) => {
		tasks.total += task_count;
		tasks.completed += status === 'Completed' ? task_count : completed_task_count;
		if (status === 'Completed') {
			tasks.timeOfLastFinishedJob = completed_at || '';
		}
	});

	return tasks;
}

function niceActionName(action: string, completed: boolean, job?: JobReport) {
	const name = job?.metadata?.location?.name || 'Unknown';
	switch (action) {
		case 'scan_location':
			return completed ? `Added location "${name}"` : `Adding location "${name}"`;
		case 'scan_location_sub_path':
			return completed ? `Indexed new files "${name}"` : `Adding location "${name}"`;
	}
	return action;
}

export default JobGroup;
