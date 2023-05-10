import { Folder } from '@sd/assets/icons';
import clsx from 'clsx';
import { X } from 'phosphor-react';
import { useMemo, useState } from 'react';
import { JobReport } from '@sd/client';
import { Button, ProgressBar, Tooltip } from '@sd/ui';
import Job from './Job';
import { useTotalElapsedTimeText } from './useGroupJobTimeText';
import { IJobGroup } from './useGroupedJobs';
import dayjs from 'dayjs';

interface JobGroupProps {
	data: IJobGroup;
	clearJob: (arg: string) => void;
}

function JobGroup({ data, clearJob }: JobGroupProps) {
	const [showChildJobs, setShowChildJobs] = useState(false);

	// running jobs should be last in the array
	const allJobs = [...data.childJobs, ...data.runningJobs];

	const isJobsRunning = allJobs.some((job) => job.status === 'Running');

	const allJobsCompleted = allJobs?.every((job) => job.status === 'Completed');

	const tasks = totalTasks(allJobs);

	const totalGroupTime = useTotalElapsedTimeText(allJobs);

	// If one job is remaining, we just delete the parent
	const clearJobHandler = (arg: string) => {
		if (data.childJobs.length === 1) {
			clearJob(data.id as string);
		} else {
			clearJob(arg);
		}
	};

	if (data.childJobs.length === 0) return <></>;

	let date_started = dayjs(data.created_at).fromNow();
	date_started = date_started.charAt(0).toUpperCase() + date_started.slice(1);

	return (
		<ul className={clsx(`relative overflow-hidden`)}>
			{allJobsCompleted && !isJobsRunning && (
				<Button
					className="absolute right-[10px] top-[19px] cursor-pointer"
					onClick={() => clearJob?.(data.id as string)}
					size="icon"
				>
					<Tooltip label="Remove">
						<X className="h-4 w-4 cursor-pointer" />
					</Tooltip>
				</Button>
			)}
			<div
				onClick={() => setShowChildJobs((v) => !v)}
				className={clsx(
					'h-auto cursor-pointer p-3 pl-4',
					showChildJobs ? 'bg-app-darkBox pb-0' : ' border-b border-app-line/50'
				)}
			>
				<div className="flex">
					<img
						src={Folder}
						className={clsx('relative left-[-2px] top-2 z-10 mr-3 h-6 w-6')}
					/>
					<div className="flex w-full flex-col">
						<div className="flex items-center">
							<div className="truncate">
								<p className="truncate font-semibold">
									{allJobsCompleted
										? `Added location "${data.metadata.init.location.name || ''
										}"`
										: 'Processing added location...'}
								</p>
								<p className="my-[2px] text-ink-faint">
									<b>{tasks.total} </b>
									{tasks.total <= 1 ? 'task' : 'tasks'}
									{" • "}
									{date_started}
									{totalGroupTime && ' • '}
									{totalGroupTime}
								</p>
							</div>
							<div className="grow" />
						</div>
						{!showChildJobs && !allJobsCompleted && (
							<div className="mt-[6px] w-full">
								<ProgressBar value={tasks.completed} total={tasks.total} />
							</div>
						)}
					</div>
				</div>
			</div>
			{showChildJobs && (
				<>
					{data.runningJobs.map((job) => (
						<Job
							className={clsx(`border-none pl-10`, showChildJobs && 'bg-app-darkBox')}
							isGroup
							key={job.id}
							job={job}
						/>
					))}
					{data.childJobs.map((job) => (
						<Job
							isGroup
							className={clsx(`border-none pl-10`, showChildJobs && 'bg-app-darkBox')}
							clearJob={clearJobHandler}
							key={job.id}
							job={job}
						/>
					))}
				</>
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

export default JobGroup;
