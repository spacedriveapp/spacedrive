/* eslint-disable tailwindcss/classnames-order */
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, CategoryHeading, PopoverClose, Tooltip } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { Trash, X } from 'phosphor-react';
import { useCallback } from 'react';
import { showAlertDialog } from '~/components/AlertDialog';
import IsRunningJob from './IsRunningJob';
import Job, { OrphanJobs } from './Job';
import JobGroup from './JobGroup';
import { useFilteredJobs } from './useFilteredJobs';
import { IJobGroup, useGroupedJobs } from './useGroupedJobs';
import { useOrphanJobs } from './useOrphanJobs';

export function JobsManager() {
	const { data: runningJobs } = useLibraryQuery(['jobs.getRunning']);
	const { data: jobs } = useLibraryQuery(['jobs.getHistory']);
	const queryClient = useQueryClient();

	const {
		individualJobs,
		runningIndividualJobs,
		jobsWithActions,
		runningJobsWithActions,
	} = useFilteredJobs(jobs, runningJobs);

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
		<div className="h-full pb-10 overflow-hidden">
			<div className="z-20 flex items-center w-full h-10 px-2 border-b rounded-t-md border-app-line/50 bg-app-button/70">
				<CategoryHeading className="ml-2">Recent Jobs</CategoryHeading>
				<div className="grow" />
				<Button onClick={() => clearAllJobsHandler()} size="icon">
					<Tooltip label="Clear out finished jobs">
						<Trash className="w-5 h-5" />
					</Tooltip>
				</Button>
				<PopoverClose asChild>
					<Button size="icon">
						<Tooltip label="Close">
							<X className="w-5 h-5" />
						</Tooltip>
					</Button>
				</PopoverClose>
			</div>
			<div className="h-full overflow-x-hidden no-scrollbar">
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
					<div className="flex items-center justify-center h-32 text-ink-dull">
						No jobs.
					</div>
				)}
			</div>
		</div>
	);
}


export { IsRunningJob };
