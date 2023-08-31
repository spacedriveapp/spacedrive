import { useQueryClient } from '@tanstack/react-query';
import { Check, Trash, X } from 'phosphor-react';
import { useJobProgress, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, PopoverClose, Tooltip, toast } from '@sd/ui';
import IsRunningJob from './IsRunningJob';
import JobGroup from './JobGroup';
import { useState } from 'react';

export function JobManager() {
	const queryClient = useQueryClient();
	const [toggleConfirmation, setToggleConfirmation] = useState(false);

	const jobGroups = useLibraryQuery(['jobs.reports']);

	const progress = useJobProgress(jobGroups.data);

	const clearAllJobs = useLibraryMutation(['jobs.clearAll'], {
		onError: () => {
			toast.error({
				title: 'Error',
				description: 'Failed to clear all jobs.'
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries(['jobs.reports ']);
			setToggleConfirmation(t => !t)
			toast.success({
				title: 'Success',
				description: 'All jobs have been cleared.'
			});
		}
	});

	const clearAllJobsHandler = () => {
		clearAllJobs.mutate(null)
	};

	return (
		<div className="h-full pb-10 overflow-hidden">
				<div className="z-20 flex items-center w-full px-2 border-b h-9 rounded-t-md border-app-line/50 bg-app-button/30">
					<span className=" ml-1.5 font-medium">Recent Jobs</span>
					<div className="grow" />
					{toggleConfirmation ? <div className="w-fit h-[85%] bg-app/40 rounded-md flex gap-2 items-center justify-center px-2 border border-app-line">
						<p className='text-[10px]'>Are you sure?</p>
						<PopoverClose asChild>
						<Check onClick={clearAllJobsHandler} className='w-3 h-3 transition-opacity duration-300 hover:opacity-70' color='white'/>
						</PopoverClose>
						<X className="w-3 h-3 transition-opacity hover:opacity-70" onClick={() => setToggleConfirmation(t => !t)} />
					</div> :
					<Button
						className="opacity-70"
						onClick={() => setToggleConfirmation(t => !t)}
						size="icon"
					>
						<Tooltip label="Clear out finished jobs">
							<Trash className="w-4 h-4" />
						</Tooltip>
					</Button>
					}
					<PopoverClose asChild>
					<Button className="opacity-70" size="icon">
						<Tooltip label="Close">
							<X className="w-4 h-4" />
						</Tooltip>
					</Button>
					</PopoverClose>
				</div>
			<div className="h-full overflow-x-hidden custom-scroll job-manager-scroll">
				<div className="h-full border-r border-app-line/50">
					{jobGroups.data &&
						(jobGroups.data.length === 0 ? (
							<div className="flex items-center justify-center h-32 text-sidebar-inkDull">
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
