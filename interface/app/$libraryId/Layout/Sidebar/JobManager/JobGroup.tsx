/* eslint-disable no-case-declarations */
import { Folder } from '@sd/assets/icons';
import { JobReport, useLibraryMutation } from '@sd/client';
import { Button, ProgressBar, Tooltip } from '@sd/ui';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { DotsThreeVertical, Pause, Play, Stop } from 'phosphor-react';
import { Fragment, useState } from 'react';
import Job from './Job';
import JobContainer from './JobContainer';
import { useTotalElapsedTimeText } from './useGroupJobTimeText';
import { IJobGroup } from './useGroupedJobs';
interface JobGroupProps {
	data: IJobGroup;
	clearJob: (arg: string) => void;
}

function JobGroup({ data: { jobs, ...data }, clearJob }: JobGroupProps) {
	const [showChildJobs, setShowChildJobs] = useState(false);

	const pauseJob = useLibraryMutation(['jobs.pause']);
	const resumeJob = useLibraryMutation(['jobs.resume']);

	const isJobsRunning = jobs.some((job) => job.status === 'Running');

	const tasks = totalTasks(jobs);
	const totalGroupTime = useTotalElapsedTimeText(jobs);

	if (!jobs.length) return <></>;

	let date_started = dayjs(jobs[0]?.created_at).fromNow();
	date_started = date_started.charAt(0).toUpperCase() + date_started.slice(1);


	return (
		<ul className="relative overflow-hidden">
			<div className='row absolute right-3 top-3 z-50 flex space-x-1'>
				{data.paused && <Button
					className="cursor-pointer"
					onClick={() => resumeJob.mutate(data.id)}
					size="icon"
					variant="outline"
				>
					<Tooltip label="Resume">
						<Play className="h-4 w-4 cursor-pointer" />
					</Tooltip>
				</Button>}


				{isJobsRunning && (<Fragment>
					<Button
						className="cursor-pointer"
						onClick={() => {
							pauseJob.mutate(data.id);
						}}
						size="icon"
						variant="outline"
					>
						<Tooltip label="Pause">
							<Pause className="h-4 w-4 cursor-pointer" />
						</Tooltip>
					</Button>
					{/* <Button
						className="cursor-pointer"
						onClick={() => resumeJob.mutate(data.id)}
						size="icon"
						variant="outline"
					>
						<Tooltip label="Stop">
							<Stop className="h-4 w-4 cursor-pointer" />
						</Tooltip>
					</Button> */}
				</Fragment>)}
				{!isJobsRunning && (
					<Button
						className="cursor-pointer"
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
			<JobContainer
				onClick={() => setShowChildJobs((v) => !v)}
				className={clsx("pb-2 hover:bg-app-selected/10", showChildJobs && "border-none bg-app-darkBox pb-1 hover:!bg-app-darkBox")}
				iconImg={Folder}
				name={niceActionName(data.action, !!data.completed, jobs[0])}
				textItems={[[{ text: `${tasks.total} ${tasks.total <= 1 ? 'task' : 'tasks'}` }, { text: date_started }, { text: data.paused ? "Paused" : data.completed ? totalGroupTime || undefined : data.queued ? "Queued" : "" }]]}
			>
				{!showChildJobs && isJobsRunning && (
					<div className="my-1 w-full">
						<ProgressBar value={tasks.completed} total={tasks.total} />
					</div>
				)}
			</JobContainer>
			{showChildJobs && (
				<div className=''>
					{jobs.map((job) => (
						<Job key={job.id} job={job} />
					))}
				</div>
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
	switch (action) {
		case 'scan_location':
			const name = job?.metadata?.init?.location?.name || 'Unknown';
			return completed ? `Added location "${name}"` : `Adding location "${name}"`;
	}
	return action;
}

export default JobGroup;
