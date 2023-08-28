import { Folder } from '@sd/assets/icons';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { DotsThreeVertical, Pause, Play, Stop } from 'phosphor-react';
import { useMemo, useState } from 'react';
import {
	JobGroup,
	JobProgressEvent,
	JobReport,
	getJobNiceActionName,
	getTotalTasks,
	useLibraryMutation,
	useTotalElapsedTimeText
} from '@sd/client';
import { Button, ProgressBar, Tooltip } from '@sd/ui';
import Job from './Job';
import JobContainer from './JobContainer';

interface JobGroupProps {
	group: JobGroup;
	progress: Record<string, JobProgressEvent>;
}

export default function ({ group, progress }: JobGroupProps) {
	const { jobs } = group;

	const [showChildJobs, setShowChildJobs] = useState(false);

	const runningJob = jobs.find((job) => job.status === 'Running');

	const tasks = getTotalTasks(jobs);
	const totalGroupTime = useTotalElapsedTimeText(jobs);

	const dateStarted = useMemo(() => {
		const createdAt = dayjs(jobs[0]?.created_at).fromNow();
		return createdAt.charAt(0).toUpperCase() + createdAt.slice(1);
	}, [jobs]);

	if (jobs.length === 0) return <></>;

	return (
		<ul className="relative overflow-hidden">
			<div className="row absolute right-3 top-3 z-50 flex space-x-1">
				<Options activeJob={runningJob} group={group} />
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
							group.action ?? '',
							group.status === 'Completed',
							jobs[0]
						)}
						textItems={[
							[
								{ text: `${tasks.total} ${tasks.total <= 1 ? 'task' : 'tasks'}` },
								{ text: dateStarted },
								{ text: totalGroupTime || undefined },

								{
									text: ['Queued', 'Paused', 'Canceled', 'Failed'].includes(
										group.status
									)
										? group.status
										: undefined
								}
							],
							[
								{
									text:
										!showChildJobs && runningJob !== undefined
											? progress[runningJob.id]?.message
											: undefined
								}
							]
						]}
					>
						{!showChildJobs && runningJob && (
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
								<Job
									isChild={jobs.length > 1}
									key={job.id}
									job={job}
									progress={progress[job.id] ?? null}
								/>
							))}
						</div>
					)}
				</>
			) : (
				<Job job={jobs[0]!} progress={progress[jobs[0]!.id] || null} />
			)}
		</ul>
	);
}

function Options({ activeJob, group }: { activeJob?: JobReport; group: JobGroup }) {
	const resumeJob = useLibraryMutation(['jobs.resume'], { onError: alert });
	const pauseJob = useLibraryMutation(['jobs.pause'], { onError: alert });
	const cancelJob = useLibraryMutation(['jobs.cancel'], { onError: alert });

	const isJobPaused = useMemo(
		() => group.jobs.some((job) => job.status === 'Paused'),
		[group.jobs]
	);

	return (
		<>
			{/* Resume */}
			{(group.status === 'Queued' || group.status === 'Paused' || isJobPaused) && (
				<Button
					className="cursor-pointer"
					onClick={() => resumeJob.mutate(group.id)}
					size="icon"
					variant="outline"
				>
					<Tooltip label="Resume">
						<Play className="h-4 w-4 cursor-pointer" />
					</Tooltip>
				</Button>
			)}
			{/* TODO: Fix this */}
			{activeJob === undefined ? (
				<Button
					className="cursor-pointer"
					// onClick={() => clearJob?.(data.id as string)}
					size="icon"
					variant="outline"
				>
					<Tooltip label="Remove">
						<DotsThreeVertical className="h-4 w-4 cursor-pointer" />
					</Tooltip>
				</Button>
			) : (
				<>
					{/* Pause / Stop */}
					<Tooltip label="Pause">
						<Button
							className="cursor-pointer"
							onClick={() => {
								pauseJob.mutate(group.id);
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
								cancelJob.mutate(group.id);
							}}
							size="icon"
							variant="outline"
						>
							<Stop className="h-4 w-4 cursor-pointer" />
						</Button>
					</Tooltip>
				</>
			)}
		</>
	);
}
