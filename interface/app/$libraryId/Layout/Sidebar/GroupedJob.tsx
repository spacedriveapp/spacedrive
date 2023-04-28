import clsx from 'clsx';
import dayjs from 'dayjs';
import { Folder, X } from 'phosphor-react';
import { MutableRefObject, memo, useEffect, useRef, useState } from 'react';
import { JobReport } from '@sd/client';
import { Button, ProgressBar, Tooltip } from '@sd/ui';
import Job from './Job';

interface GroupJobProps {
	jobs?: JobReport[];
	clearAJob?: (arg: string) => void;
	runningJobs?: JobReport[];
	parentJob?: JobReport;
}

function GroupedJob({ jobs = [], clearAJob, runningJobs = [], parentJob }: GroupJobProps) {
	const [toggleJobs, setToggleJobs] = useState<MutableRefObject<boolean> | boolean>(false);
	const toggleRef = useRef(toggleJobs);
	const checkForJobsRunning = jobs?.some((job) => job.status === 'Running');
	const allJobsCompleted = jobs?.every((job) => job.status === 'Completed');
	const filterJobsFromParent = jobs?.filter((job) => job.id !== parentJob?.id); //jobs array contains all jobs - we don't want to show the parent job in the list

	useEffect(() => {
		setToggleJobs(toggleRef.current); //this is to keep the toggled group open on re-renders
	}, []);

	//If one job, including the parent is remaining, we delete the group
	const clearJobHandler = (arg: string) => {
		if (jobs.length === 2) {
			clearAJob?.(parentJob?.id as string);
		} else {
			clearAJob?.(arg);
		}
	};

	return (
		<>
			{jobs.length === 0 ? null : (
				<ul className={clsx(`relative overflow-hidden`, toggleJobs && 'groupjobul')}>
					{allJobsCompleted && !checkForJobsRunning && (
						<Button
							className="absolute right-[10px] top-[30px] cursor-pointer"
							onClick={() => clearAJob?.(parentJob?.id as string)}
							size="icon"
						>
							<Tooltip label="Remove">
								<X className="h-4 w-4 cursor-pointer" />
							</Tooltip>
						</Button>
					)}
					<div
						onClick={() => setToggleJobs(!toggleJobs)}
						className={clsx(
							'h-auto cursor-pointer p-3 pl-4',
							toggleJobs ? 'darker-app-bg pb-0' : ' border-b border-app-line/50'
						)}
					>
						<div className="flex">
							<Folder className={clsx('relative top-2 mr-3 h-5 w-5')} />
							<div className="flex w-full flex-col">
								<div className="flex items-center">
									<div className="truncate">
										<span className="truncate font-semibold">
											{allJobsCompleted
												? `Added location ${
														parentJob?.metadata.init.location.name || ''
												  }`
												: 'Processing added location...'}
										</span>
										<p className="mt-[2px] mb-[5px] text-[12px] italic text-ink-faint">
											{getTotalTasks(jobs).total} tasks
										</p>
										<div className="flex gap-1 truncate text-ink-faint">
											<GetTotalGroupJobTime jobs={jobs} />
											{/* {allJobsCompleted && (
												<span className="text-xs">
													- Took{' '}
													{dayjs(timeOfLastFinishedJob).fromNow(true)}
												</span>
											)} */}
										</div>
									</div>
									<div className="grow" />
								</div>
								{!allJobsCompleted && !toggleJobs && (
									<div className="mt-[6px] w-full">
										<ProgressBar
											value={getTotalTasks(jobs).completed}
											total={getTotalTasks(jobs).total}
										/>
									</div>
								)}
							</div>
						</div>
					</div>
					{toggleJobs && (
						<>
							{runningJobs?.map((job) => (
								<Job
									className={clsx(
										`border-none pl-10`,
										toggleJobs && 'darker-app-bg'
									)}
									isGroup={true}
									key={job.id}
									job={job}
								/>
							))}
							{filterJobsFromParent?.map((job) => (
								<Job
									isGroup={true}
									className={clsx(
										`border-none pl-10`,
										toggleJobs && 'darker-app-bg'
									)}
									clearAJob={(arg) => clearJobHandler(arg)}
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
