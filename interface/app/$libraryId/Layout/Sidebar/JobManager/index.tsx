/* eslint-disable tailwindcss/classnames-order */
import { useQueryClient } from '@tanstack/react-query';
import { Trash, X } from 'phosphor-react';
import { useCallback } from 'react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, CategoryHeading, PopoverClose, Tooltip } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import IsRunningJob from './IsRunningJob';
import Job from './Job';
import JobGroup from './JobGroup';
import { useFilteredJobs } from './useFilteredJobs';
import { useGroupedJobs } from './useGroupedJobs';
import { useOrphanJobs } from './useOrphanJobs';

export function JobsManager() {
	const { data: runningJobs } = useLibraryQuery(['jobs.getRunning']);
	const { data: jobs } = useLibraryQuery(['jobs.getHistory']);
	const queryClient = useQueryClient();

	const { individualJobs, runningIndividualJobs, jobsWithActions, runningJobsWithActions } =
		useFilteredJobs(jobs, runningJobs);

	const groupedJobs = useGroupedJobs(jobsWithActions, runningJobsWithActions);

	const orphanJobs = useOrphanJobs(jobsWithActions, runningJobsWithActions);

	const clearAllJobs = useLibraryMutation(['jobs.clearAll'], {
		onError: () => {
			showAlertDialog({
				title: 'Error',
				value: 'There was an error clearing all jobs. Please try again.'
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries(['jobs.getHistory']);
		}
	});
	const clearJob = useLibraryMutation(['jobs.clear'], {
		onError: () => {
			showAlertDialog({
				title: 'Error',
				value: 'There was an error clearing the job. Please try again.'
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries(['jobs.getHistory']);
		}
	});

	const clearAllJobsHandler = () => {
		showAlertDialog({
			title: 'Clear Jobs',
			value: 'Are you sure you want to clear all jobs? This cannot be undone.',
			label: 'Clear',
			onSubmit: () => clearAllJobs.mutate(null)
		});
	};
	const clearJobHandler = useCallback(
		(id: string) => {
			clearJob.mutate(id);
		},
		[clearJob]
	);

	return (
		<div className="h-full overflow-hidden pb-10">
			<div className="z-20 flex h-10 w-full items-center rounded-t-md border-b border-app-line/50 bg-app-button/70 px-2">
				<CategoryHeading className="ml-2">Recent Jobs</CategoryHeading>
				<div className="grow" />
				<Button onClick={() => clearAllJobsHandler()} size="icon">
					<Tooltip label="Clear out finished jobs">
						<Trash className="h-5 w-5" />
					</Tooltip>
				</Button>
				<PopoverClose asChild>
					<Button size="icon">
						<Tooltip label="Close">
							<X className="h-5 w-5" />
						</Tooltip>
					</Button>
				</PopoverClose>
			</div>
			<div className="no-scrollbar h-full overflow-x-hidden">
				{groupedJobs.map((data) => (
					<JobGroup key={data.id} data={data} clearJob={clearJobHandler} />
				))}
				{orphanJobs?.map((job) => (
					<Job key={job?.id} job={job} />
				))}
				{runningIndividualJobs?.map((job) => (
					<Job key={job.id} job={job} />
				))}
				{individualJobs?.map((job) => (
					<Job clearJob={clearJobHandler} key={job.id} job={job} />
				))}
				{jobs?.length === 0 && runningJobs?.length === 0 && (
					<div className="flex h-32 items-center justify-center text-ink-dull">
						No jobs.
					</div>
				)}
			</div>
		</div>
	);
}

export { IsRunningJob };
