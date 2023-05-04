import clsx from 'clsx';
import dayjs from 'dayjs';
import { Folder, TextItalic, X } from 'phosphor-react';
import { MutableRefObject, memo, useEffect, useState } from 'react';
import { JobReport } from '@sd/client';
import { Button, ProgressBar, Tooltip } from '@sd/ui';
import { IGroupedJobs } from './GroupedJobs';
import Job from './Job';

interface GroupJobProps {
	data: IGroupedJobs;
	clearJob: (arg: string) => void;
}

function GroupedJob({ data, clearJob }: GroupJobProps) {
	const [toggleJobs, setToggleJobs] = useState<MutableRefObject<boolean> | boolean>(false);
	const checkForJobsRunning = data.childJobs?.some((job) => job.status === 'Running');
	const allJobsCompleted = data.childJobs?.every((job) => job.status === 'Completed');
	const getTasks = getTotalTasks(data.childJobs);
	//If one job is remaining, we just delete the parent
	const clearJobHandler = (arg: string) => {
		if (data.childJobs.length === 1) {
			clearJob(data.id as string);
		} else {
			clearJob(arg);
		}
	};

	return (
		<>
			{data.childJobs.length === 0 ? null : (
				<ul className={clsx(`relative overflow-hidden`, toggleJobs && 'groupjobul')}>
					{allJobsCompleted && !checkForJobsRunning && (
						<Button
							className="absolute right-[10px] top-[30px] cursor-pointer"
							onClick={() => clearJob?.(data.id as string)}
							size="icon"
						>
							<Tooltip label="Remove">
								<X className="h-4 w-4 cursor-pointer" />
							</Tooltip>
						</Button>
					)}
					<div
						onClick={() => setToggleJobs((v) => !v)}
						className={clsx(
							'h-auto cursor-pointer p-3 pl-4',
							toggleJobs ? 'bg-app-darkBox pb-0' : ' border-b border-app-line/50'
						)}
					>
						<div className="flex">
							<Folder
								className={clsx(
									'relative left-[-2px] top-2 mr-3 h-6 w-6 rounded-full bg-app-button p-[5.5px]'
								)}
							/>
							<div className="flex w-full flex-col">
								<div className="flex items-center">
									<div className="truncate">
										<p className="truncate font-semibold">
											{allJobsCompleted
												? `Added location "${
														data.metadata.init.location.name || ''
												  }"`
												: 'Processing added location...'}
										</p>
										<p className="mb-[5px] mt-[2px] text-[12px] italic text-ink-faint">
											{getTasks.total}{' '}
											{getTasks.total <= 1 ? 'task' : 'tasks'}
										</p>
										<div className="flex gap-1 truncate text-ink-faint">
											<GetTotalGroupJobTime jobs={data.childJobs} />
										</div>
									</div>
									<div className="grow" />
								</div>
								{!allJobsCompleted && !toggleJobs && (
									<div className="mt-[6px] w-full">
										<ProgressBar
											value={getTasks.completed}
											total={getTasks.total}
										/>
									</div>
								)}
							</div>
						</div>
					</div>
					{toggleJobs && (
						<>
							{data.runningJobs.map((job) => (
								<Job
									className={clsx(
										`border-none pl-10`,
										toggleJobs && 'bg-app-darkBox'
									)}
									isGroup
									key={job.id}
									job={job}
								/>
							))}
							{data.childJobs.map((job) => (
								<Job
									isGroup
									className={clsx(
										`border-none pl-10`,
										toggleJobs && 'bg-app-darkBox'
									)}
									clearJob={clearJobHandler}
									key={job.id}
									job={job}
								/>
							))}
						</>
					)}
				</ul>
			)}
		</>
	);
}

function getTotalTasks(jobs: JobReport[]) {
	const task = { completed: 0, total: 0, timeOfLastFinishedJob: '' };
	jobs?.forEach((job) => {
		task.total += job.task_count;
		task.completed += job.status === 'Completed' ? job.task_count : 0;
		if (job.status === 'Completed') {
			task.timeOfLastFinishedJob = job.completed_at || '';
		}
	});
	return {
		completed: task.completed,
		total: task.total,
		timeOfLastFinishedJob: task.timeOfLastFinishedJob
	};
}

function GetTotalGroupJobTime({ jobs }: { jobs?: JobReport[] }) {
	const [_, setRerenderPlz] = useState(0);
	const allJobsCompleted = jobs?.every((job) => job.completed_at);
	const checkForJobsRunning = jobs?.some((job) => job.status === 'Running');
	const checkForJobsQueued = jobs?.some((job) => job.status === 'Queued');
	const checkIfJobsFailedOrCancelled = jobs?.some(
		(job) => job.status === 'Failed' || job.status === 'Canceled'
	);
	let total = 0;
	let text;
	jobs?.forEach((job) => {
		if (
			!allJobsCompleted &&
			!checkForJobsQueued &&
			!checkForJobsRunning &&
			!checkIfJobsFailedOrCancelled
		) {
			const start = dayjs(job.started_at);
			const end = dayjs(job.completed_at);
			total += end.diff(start, 'minutes');
			text = `Took ${dayjs(total).from(end, true)}`;
		} else if (
			checkForJobsRunning &&
			checkForJobsQueued &&
			!allJobsCompleted &&
			!checkIfJobsFailedOrCancelled
		) {
			const start = dayjs(job.started_at);
			const end = dayjs();
			total += end.diff(start, 'minutes');
			text = `Elapsed in ${dayjs(total).fromNow(true)}`;
		} else if (checkIfJobsFailedOrCancelled) {
			text = `Job failed or canceled`;
		} else {
			text = `Elapsed in ${dayjs(job.created_at).fromNow(true)}`;
		}
	});
	useEffect(() => {
		if (!allJobsCompleted || checkForJobsQueued) {
			const interval = setInterval(() => {
				setRerenderPlz((x) => x + 1); // Trigger React to rerender and dayjs to update
			}, 1000);
			return () => clearInterval(interval);
		}
	}, [allJobsCompleted, checkForJobsQueued]);
	return <span className="text-xs">{text}</span>;
}

export default memo(GroupedJob);
