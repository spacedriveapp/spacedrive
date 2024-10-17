import { DotsThreeVertical, Eye, Pause, Play, Stop, Trash } from '@phosphor-icons/react';
import { Folder } from '@sd/assets/icons';
import { useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { useMemo, useState } from 'react';

import {
	formatNumber,
	getJobNiceActionName,
	getTotalTasks,
	JobGroup,
	JobProgressEvent,
	Report,
	useLibraryMutation,
	useTotalElapsedTimeText
} from '@sd/client';
import { Button, Dropdown, ProgressBar, toast, Tooltip } from '@sd/ui';
import { useLocale } from '~/hooks';

import Job from './Job';
import JobContainer from './JobContainer';

interface JobGroupProps {
	group: JobGroup;
	progress: Record<string, JobProgressEvent>;
}

export default function ({ group, progress }: JobGroupProps) {
	const { jobs } = group;

	const [showChildJobs, setShowChildJobs] = useState(false);

	const runningJob = jobs.find((job: { status: string }) => job.status === 'Running');

	const tasks = getTotalTasks(jobs);
	const totalGroupTime = useTotalElapsedTimeText(jobs);

	const dateStarted = useMemo(() => {
		const createdAt = dayjs(jobs[0]?.created_at).fromNow();
		return createdAt.charAt(0).toUpperCase() + createdAt.slice(1);
	}, [jobs]);

	if (jobs.length === 0) return <></>;
	const { t } = useLocale();

	const calculateETA = (job: Report) => {
		let diff = 0;
		if (job.created_at && job.estimated_completion) {
			const start = new Date(job.created_at);
			const end = new Date(job.estimated_completion);
			diff = Math.abs(end.getTime() - start.getTime());
		}
		return diff;
	};

	return (
		<ul className="relative overflow-visible">
			<div className="row absolute right-3 top-3 z-50 flex space-x-1">
				<Options
					showChildJobs={showChildJobs}
					setShowChildJobs={() => setShowChildJobs(v => !v)}
					activeJob={runningJob}
					group={group}
				/>
			</div>
			{jobs?.length > 1 ? (
				<>
					<JobContainer
						onClick={() => setShowChildJobs(v => !v)}
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
								{
									text: `${formatNumber(tasks.total)} ${t('task', { count: tasks.total })}`
								},
								{ text: dateStarted },
								{ text: totalGroupTime || undefined },

								{
									text: ['Queued', 'Paused', 'Canceled', 'Failed'].includes(
										group.status
									)
										? t(`${group.status.toLowerCase()}`)
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
							{jobs.map(job => {
								const diff = calculateETA(job);

								return (
									<Job
										isChild={jobs.length > 1}
										key={job.id}
										job={job}
										progress={progress[job.id] ?? null}
										eta={diff}
									/>
								);
							})}
						</div>
					)}
				</>
			) : (
				// add eta for individual jobs
				<Job
					job={jobs[0]!}
					progress={progress[jobs[0]!.id] || null}
					eta={calculateETA(jobs[0]!)}
				/>
			)}
		</ul>
	);
}

const toastErrorSuccess = (
	errorMessage?: string,
	successMessage?: string,
	successCallBack?: () => void
) => {
	return {
		onError: () => {
			if (errorMessage)
				toast.error({
					title: 'Error',
					body: errorMessage
				});
		},
		onSuccess: () => {
			if (successMessage)
				toast.success({
					title: 'Success',
					body: successMessage
				});
			successCallBack?.();
		}
	};
};

function Options({
	activeJob,
	group,
	setShowChildJobs,
	showChildJobs
}: {
	activeJob?: Report;
	group: JobGroup;
	setShowChildJobs: () => void;
	showChildJobs: boolean;
}) {
	const queryClient = useQueryClient();

	const { t } = useLocale();

	const resumeJob = useLibraryMutation(
		['jobs.resume'],
		toastErrorSuccess(t('failed_to_resume_job'), t('job_has_been_resumed'))
	);
	const pauseJob = useLibraryMutation(
		['jobs.pause'],
		toastErrorSuccess(t('failed_to_pause_job'), t('job_has_been_paused'))
	);
	const cancelJob = useLibraryMutation(
		['jobs.cancel'],
		toastErrorSuccess(t('failed_to_cancel_job'), t('job_has_been_canceled'))
	);
	const clearJob = useLibraryMutation(
		['jobs.clear'],
		toastErrorSuccess(t('failed_to_remove_job'), undefined, () => {
			queryClient.invalidateQueries({ queryKey: ['jobs.reports'] });
		})
	);

	const clearJobHandler = () => {
		group.jobs.forEach(job => {
			clearJob.mutate(job.id);
			//only one toast for all jobs
			if (job.id === group.id)
				toast.success({ title: t('success'), body: t('job_has_been_removed') });
		});
	};

	const isJobPaused = useMemo(
		() => group.jobs.some(job => job.status === 'Paused'),
		[group.jobs]
	);

	return (
		<>
			{/* Resume */}
			{(group.status === 'Queued' || group.status === 'Paused' || isJobPaused) && (
				<Button
					className="cursor-pointer"
					onClick={() =>
						resumeJob.mutate(
							group.running_job_id != null ? group.running_job_id : group.id
						)
					}
					size="icon"
					variant="outline"
				>
					<Tooltip label={t('resume')}>
						<Play className="size-4 cursor-pointer" />
					</Tooltip>
				</Button>
			)}
			{activeJob === undefined ? (
				<Dropdown.Root
					align="right"
					itemsClassName="!bg-app-darkBox !border-app-box !top-[-8px]"
					button={
						<Tooltip label={t('actions')}>
							<Button className="!px-1" variant="outline">
								<DotsThreeVertical className="size-4 cursor-pointer" />
							</Button>
						</Tooltip>
					}
				>
					<Dropdown.Section>
						{group.jobs.length > 1 && (
							<Dropdown.Item
								active={showChildJobs}
								onClick={setShowChildJobs}
								icon={Eye}
								iconClassName="!w-3"
								className="!text-[11px] text-ink-dull"
							>
								{t('expand')}
							</Dropdown.Item>
						)}
						<Dropdown.Item
							onClick={() => clearJobHandler()}
							icon={Trash}
							iconClassName="!w-3"
							className="!text-[11px] text-ink-dull"
						>
							{t('remove')}
						</Dropdown.Item>
					</Dropdown.Section>
				</Dropdown.Root>
			) : (
				<>
					{/* Pause / Stop */}
					<Tooltip label={t('pause')}>
						<Button
							className="cursor-pointer"
							onClick={() =>
								pauseJob.mutate(
									group.running_job_id != null ? group.running_job_id : group.id
								)
							}
							size="icon"
							variant="outline"
						>
							<Pause className="size-4 cursor-pointer" />
						</Button>
					</Tooltip>
					<Tooltip label={t('stop')}>
						<Button
							className="cursor-pointer"
							onClick={() => {
								cancelJob.mutate(
									group.running_job_id != null ? group.running_job_id : group.id
								);
							}}
							size="icon"
							variant="outline"
						>
							<Stop className="size-4 cursor-pointer" />
						</Button>
					</Tooltip>
				</>
			)}
		</>
	);
}
