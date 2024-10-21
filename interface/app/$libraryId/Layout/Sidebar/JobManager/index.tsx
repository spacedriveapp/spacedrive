import { Check, PushPin, Trash, X } from '@phosphor-icons/react';
import { useQueryClient } from '@tanstack/react-query';
import dayjs from 'dayjs';
import { useState } from 'react';
import {
	JobGroup as IJobGroup,
	Report,
	useJobProgress,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/client';
import { Button, PopoverClose, toast, Tooltip } from '@sd/ui';
import { useIsDark, useLocale } from '~/hooks';

import { useSidebarContext } from '../SidebarLayout/Context';
import { getSidebarStore, useSidebarStore } from '../store';
import IsRunningJob from './IsRunningJob';
import JobGroup from './JobGroup';

const sortByCreatedAt = (a: IJobGroup, b: IJobGroup) => {
	const aDate = dayjs(a.created_at);
	const bDate = dayjs(b.created_at);
	if (aDate.isBefore(bDate)) {
		return 1;
	} else if (bDate.isBefore(aDate)) {
		return -1;
	}
	return 0;
};

function sortJobData(jobs: IJobGroup[]) {
	const runningJobs: IJobGroup[] = [];
	const otherJobs: IJobGroup[] = [];

	jobs.forEach((job) => {
		if (job.status === 'Running' || job.jobs.find((job) => job.status === 'Running')) {
			runningJobs.push(job);
		} else {
			otherJobs.push(job);
		}
	});

	runningJobs.sort(sortByCreatedAt);
	otherJobs.sort(sortByCreatedAt);

	return [...runningJobs, ...otherJobs];
}

export function JobManager() {
	const queryClient = useQueryClient();
	const [toggleConfirmation, setToggleConfirmation] = useState(false);
	const store = useSidebarStore();

	const sidebar = useSidebarContext();

	const jobGroups = useLibraryQuery(['jobs.reports']);

	const progress = useJobProgress(jobGroups.data);

	const isDark = useIsDark();

	const { t } = useLocale();

	const clearJob = useLibraryMutation(['jobs.clear']);

	const clearAllJobsHandler = async () => {
		try {
			const clearPromises: Promise<null>[] = [];
			jobGroups.data?.forEach((group: IJobGroup) => {
				if (group.jobs.length > 1) {
					let allComplete = true;
					group.jobs.forEach((job: Report) => {
						if (job.status !== 'Completed' && job.status !== 'CompletedWithErrors') {
							allComplete = false;
						}
					});
					if (allComplete) {
						group.jobs.forEach((job: Report) => {
							clearPromises.push(clearJob.mutateAsync(job.id));
						});
					}
				} else {
					if (
						group.status === 'Completed' ||
						group.status === 'CompletedWithErrors' ||
						group.status === 'Canceled' ||
						group.status === 'Failed'
					) {
						clearPromises.push(clearJob.mutateAsync(group.id));
					}
				}
			});
			await Promise.all(clearPromises);

			setToggleConfirmation((t) => !t);
			toast.success({
				title: t('success'),
				body: t('all_jobs_have_been_cleared')
			});
			queryClient.invalidateQueries({ queryKey: ['jobs.reports'] });
		} catch (error) {
			toast.error({
				title: t('error'),
				body: t('failed_to_clear_all_jobs')
			});
		}
	};

	return (
		<div className="h-full overflow-hidden pb-10">
			<div className="z-20 flex h-9 w-full items-center rounded-t-md border-b border-app-line/50 bg-app-button/30 px-2">
				{!sidebar.collapsed && (
					<Tooltip label={t('pin')}>
						<Button
							onClick={() => {
								getSidebarStore().pinJobManager = !store.pinJobManager;
							}}
							size="icon"
						>
							<PushPin weight={store.pinJobManager ? 'fill' : 'regular'} size={16} />
						</Button>
					</Tooltip>
				)}
				<span className="ml-1 font-plex font-semibold tracking-wide">
					{t('recent_jobs')}
				</span>
				<div className="grow" />
				{toggleConfirmation ? (
					<div className="flex h-[85%] w-fit items-center justify-center gap-2 rounded-md border border-app-line bg-app/40 px-2">
						<p className="text-[10px]">{t('are_you_sure')}</p>
						<PopoverClose asChild>
							<Check
								onClick={clearAllJobsHandler}
								className="size-3 transition-opacity duration-300 hover:opacity-70"
								color={isDark ? 'white' : 'black'}
							/>
						</PopoverClose>
						<X
							className="size-3 transition-opacity hover:opacity-70"
							onClick={() => setToggleConfirmation((t) => !t)}
						/>
					</div>
				) : (
					<Button
						className="opacity-70"
						onClick={() => setToggleConfirmation((t) => !t)}
						size="icon"
					>
						<Tooltip label={t('clear_finished_jobs')}>
							<Trash size={16} />
						</Tooltip>
					</Button>
				)}
				<PopoverClose asChild>
					<Button
						onClick={() => (getSidebarStore().pinJobManager = false)}
						className="opacity-70"
						size="icon"
					>
						<Tooltip label={t('close')}>
							<X size={16} />
						</Tooltip>
					</Button>
				</PopoverClose>
			</div>
			<div className="custom-scroll job-manager-scroll h-full overflow-x-hidden">
				<div className="h-full border-r border-app-line/50">
					{jobGroups.data &&
						(jobGroups.data.length === 0 ? (
							<div className="flex h-32 items-center justify-center font-plex text-sidebar-inkDull">
								{t('no_jobs')}
							</div>
						) : (
							sortJobData(jobGroups.data).map((group) => (
								<JobGroup key={group.id} group={group} progress={progress} />
							))
						))}
				</div>
			</div>
		</div>
	);
}

export { IsRunningJob };
