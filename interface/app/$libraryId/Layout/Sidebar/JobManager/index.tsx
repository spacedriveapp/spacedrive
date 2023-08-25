import { useQueryClient } from '@tanstack/react-query';
import { Trash, X } from 'phosphor-react';
import { useJobProgress, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, PopoverClose, Tooltip } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import IsRunningJob from './IsRunningJob';
import JobGroup from './JobGroup';

export function JobManager() {
	const queryClient = useQueryClient();

	const jobGroups = useLibraryQuery(['jobs.reports']);

	const progress = useJobProgress(jobGroups.data);

	const clearAllJobs = useLibraryMutation(['jobs.clearAll'], {
		onError: () => {
			showAlertDialog({
				title: 'Error',
				value: 'There was an error clearing all jobs. Please try again.'
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries(['jobs.reports ']);
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
				<div className="z-20 flex h-9 w-full items-center rounded-t-md border-b border-app-line/50 bg-app-button/30 px-2">
					<span className=" ml-1.5 font-medium">Recent Jobs</span>

					<div className="grow" />
					<Button
						className="opacity-70"
						onClick={() => clearAllJobsHandler()}
						size="icon"
					>
						<Tooltip label="Clear out finished jobs">
							<Trash className="h-4 w-4" />
						</Tooltip>
					</Button>
					<Button className="opacity-70" size="icon">
						<Tooltip label="Close">
							<X className="h-4 w-4" />
						</Tooltip>
					</Button>
				</div>
			</PopoverClose>
			<div className="custom-scroll job-manager-scroll h-full overflow-x-hidden">
				<div className="h-full border-r border-app-line/50">
					{jobGroups.data &&
						(jobGroups.data.length === 0 ? (
							<div className="flex h-32 items-center justify-center text-sidebar-inkDull">
								No jobs.
							</div>
						) : (
							jobGroups.data.map((group) => (
								<JobGroup key={group.id} group={group} progress={progress} />
							))
						))}
				</div>
			</div>
		</div>
	);
}

export { IsRunningJob };
