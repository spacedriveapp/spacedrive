import { useQueryClient } from '@tanstack/react-query';
import { Trash, X } from 'phosphor-react';
import { useCallback } from 'react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, CategoryHeading, PopoverClose, Tooltip } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import IsRunningJob from './IsRunningJob';
import Job from './Job';
import JobGroup from './JobGroup';
import { useGroupedJobs } from './useGroupedJobs';
import dayjs from 'dayjs';

export function JobsManager() {
	const { data: _runningJobs } = useLibraryQuery(['jobs.getRunning']);
	const { data: _jobs } = useLibraryQuery(['jobs.getHistory']);
	const queryClient = useQueryClient();

	// this might need memoization
	const jobs = [...(_jobs || []), ...(_runningJobs || [])].sort((a, b) => {
		return dayjs(a.created_at).isBefore(dayjs(b.created_at)) ? 1 : -1;
	});

	const groupedJobs = useGroupedJobs(jobs);

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

	const clearAllJobsHandler = () => {
		showAlertDialog({
			title: 'Clear Jobs',
			value: 'Are you sure you want to clear all jobs? This cannot be undone.',
			label: 'Clear',
			onSubmit: () => clearAllJobs.mutate(null)
		});
	};

	return (
		<div className="h-full overflow-hidden pb-10">
			<PopoverClose asChild>
				<div className="z-20 flex h-9 w-full items-center rounded-t-md border-b border-app-line/50 bg-app-button/70 px-2">
					<CategoryHeading className="ml-1.5 font-medium">Recent Jobs</CategoryHeading>
					<div className='mx-2 flex h-4 w-4 items-center justify-center rounded-full bg-app-selected text-tiny font-medium text-ink/60'>{groupedJobs.length}</div>
					<div className="grow" />
					<Button className='opacity-70' onClick={() => clearAllJobsHandler()} size="icon">
						<Tooltip label="Clear out finished jobs">
							<Trash className="h-4 w-4" />
						</Tooltip>
					</Button>
					<Button className='opacity-70' size="icon">
						<Tooltip label="Close">
							<X className="h-4 w-4" />
						</Tooltip>
					</Button>
				</div>
			</PopoverClose>
			<div className="custom-scroll job-manager-scroll h-full overflow-x-hidden">
				<div className='h-full border-r border-app-line/50'>
					{groupedJobs?.map((group) => (
						<JobGroup key={group.id} data={group} clearJob={function (arg: string): void {
							throw new Error('Function not implemented.');
						}} />
					))}
					{jobs?.length === 0 && (
						<div className="flex h-32 items-center justify-center text-sidebar-inkDull">
							No jobs.
						</div>
					)}
				</div>
			</div>
		</div>
	);
}

export { IsRunningJob };
