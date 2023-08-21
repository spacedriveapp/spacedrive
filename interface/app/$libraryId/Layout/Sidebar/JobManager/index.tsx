import { useQueryClient } from '@tanstack/react-query';
import { Trash, X } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import {
	JobGroup as JobGroupType,
	JobProgressEvent,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription
} from '@sd/client';
import { Button, PopoverClose, Tooltip } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import IsRunningJob from './IsRunningJob';
import JobGroup from './JobGroup';
import { useJobManagerContext } from './context';

export function JobManager() {
	const queryClient = useQueryClient();

	const jobGroups = useLibraryQuery(['jobs.reports']);

	const progress = useProgress(jobGroups.data);

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

const useProgress = (jobGroups?: JobGroupType[]) => {
	const ctx = useJobManagerContext();

	// Create initial progress from cached progress
	const [progress, setProgress] = useState<Record<string, JobProgressEvent>>(() => {
		return {
			...ctx.cachedJobProgress.current
		};
	});

	useLibrarySubscription(['jobs.progress'], {
		onData(data) {
			console.log(`setting ${data.id} progress`);
			setProgress((prev) => ({ ...prev, [data.id]: data }));
		}
	});

	// Update cached progress when progress changes
	useEffect(() => {
		ctx.cachedJobProgress.current = progress;
	}, [progress, ctx.cachedJobProgress]);

	// Remove jobs that aren't running from progress
	// This can happen kind of lazily since it's not a huge deal
	useEffect(() => {
		if (!jobGroups) return;

		setProgress((prev) => {
			const ret: typeof prev = {};

			for (const group of jobGroups) {
				for (const job of group.jobs) {
					const prevEvent = prev[job.id];
					if (job.status !== 'Running' || !prevEvent) continue;

					ret[job.id] = prevEvent;
				}
			}

			return ret;
		});
	}, [jobGroups]);

	return progress;
};
