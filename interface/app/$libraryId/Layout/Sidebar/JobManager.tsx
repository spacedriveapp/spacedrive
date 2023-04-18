import { useQueryClient } from '@tanstack/react-query';
import { Trash, X } from 'phosphor-react';
import { useCallback } from 'react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, CategoryHeading, PopoverClose, Tooltip } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import Job from './Job';

export function JobsManager() {
	const { data: runningJobs } = useLibraryQuery(['jobs.getRunning']);
	const { data: jobs } = useLibraryQuery(['jobs.getHistory']);
	const queryClient = useQueryClient();
	const { mutate: clearAllJobs } = useLibraryMutation(['jobs.clearAll'], {
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
	const { mutate: clearAJob } = useLibraryMutation(['jobs.clear'], {
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

	const jobsToFilter = [
		'shallow_thumbnailer',
		'shallow_indexer',
		'shallow_file_identifier',
		'indexer',
		'thumbnailer'
	];
	const updatedJobsWithFilter = jobs?.filter((job) => !jobsToFilter.includes(job.name));

	const runningJobsToFilter = ['indexer'];
	const updatedRunningJobsWithFilter = runningJobs?.filter(
		(job) => !runningJobsToFilter.includes(job.name)
	);

	console.log(updatedJobsWithFilter);

	const clearAllJobsHandler = () => {
		showAlertDialog({
			title: 'Clear all jobs?',
			value: 'Are you sure you want to clear all jobs? This cannot be undone.',
			onSubmit: () => clearAllJobs(null)
		});
	};
	const clearAJobHandler = useCallback(
		(id: string) => {
			clearAJob(id);
		},
		[clearAJob]
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
			<div className="custom-scroll inspector-scroll mr-1 h-full overflow-x-hidden">
				<div className="">
					<div className="py-1">
						{updatedRunningJobsWithFilter?.map((job) => (
							<Job key={job.id} job={job} />
						))}
						{updatedJobsWithFilter?.map((job) => (
							<Job
								clearAJob={(arg: string) => clearAJobHandler(arg)}
								key={job.id}
								job={job}
							/>
						))}
						{updatedJobsWithFilter?.length === 0 &&
							updatedRunningJobsWithFilter?.length === 0 && (
								<div className="flex h-32 items-center justify-center text-ink-dull">
									No jobs.
								</div>
							)}
					</div>
				</div>
			</div>
		</div>
	);
}
