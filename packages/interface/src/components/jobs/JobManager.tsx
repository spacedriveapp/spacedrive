import clsx from 'clsx';
import dayjs from 'dayjs';
import {
	ArrowsClockwise,
	Camera,
	Eye,
	Fingerprint,
	Folder,
	LockSimple,
	LockSimpleOpen,
	Pause,
	Question,
	Trash,
	TrashSimple,
	X
} from 'phosphor-react';
import { JobReport, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, CategoryHeading, Popover, PopoverClose, tw } from '@sd/ui';
import ProgressBar from '../primitive/ProgressBar';
import { Tooltip } from '../tooltip/Tooltip';

interface JobNiceData {
	name: string;
	icon: React.ForwardRefExoticComponent<any>;
}

const getNiceData = (job: JobReport): Record<string, JobNiceData> => ({
	indexer: {
		name: `Indexed ${numberWithCommas(job.metadata?.data?.total_paths || 0)} paths at "${
			job.metadata?.data?.location_path || '?'
		}"`,
		icon: Folder
	},
	thumbnailer: {
		name: `Generated ${numberWithCommas(job.task_count)} thumbnails`,
		icon: Camera
	},
	file_identifier: {
		name: `Extracted metadata for ${numberWithCommas(job.metadata?.total_orphan_paths || 0)} files`,
		icon: Eye
	},
	object_validator: {
		name: `Generated ${numberWithCommas(job.task_count)} full object hashes`,
		icon: Fingerprint
	},
	file_encryptor: {
		name: `Encrypted ${numberWithCommas(job.task_count)} ${
			job.task_count > 1 || job.task_count === 0 ? 'files' : 'file'
		}`,
		icon: LockSimple
	},
	file_decryptor: {
		name: `Decrypted ${numberWithCommas(job.task_count)} ${
			job.task_count > 1 || job.task_count === 0 ? 'files' : 'file'
		}`,
		icon: LockSimpleOpen
	},
	file_eraser: {
		name: `Securely erased ${numberWithCommas(job.task_count)} ${
			job.task_count > 1 || job.task_count === 0 ? 'files' : 'file'
		}`,
		icon: TrashSimple
	},
	file_deleter: {
		name: `Deleted ${numberWithCommas(job.task_count)} ${
			job.task_count > 1 || job.task_count === 0 ? 'files' : 'file'
		}`,
		icon: Trash
	}
});

const StatusColors: Record<JobReport['status'], string> = {
	Running: 'text-blue-500',
	Failed: 'text-red-500',
	Completed: 'text-green-500',
	Queued: 'text-yellow-500',
	Canceled: 'text-gray-500',
	Paused: 'text-gray-500'
};

function elapsed(seconds: number) {
	return new Date(seconds * 1000).toUTCString().match(/(\d\d:\d\d:\d\d)/)?.[0];
}

const HeaderContainer = tw.div`z-20 flex items-center w-full h-10 px-2 border-b border-app-line/50 rounded-t-md bg-app-button/70`;

export function JobsManager() {
	const runningJobs = useLibraryQuery(['jobs.getRunning']);
	const jobs = useLibraryQuery(['jobs.getHistory']);
	const clearAllJobs = useLibraryMutation(['jobs.clearAll']);

	return (
		<div className="h-full pb-10 overflow-hidden">
			<HeaderContainer>
				<CategoryHeading className="ml-2">Recent Jobs</CategoryHeading>
				<div className="flex-grow" />

				<Button onClick={() => clearAllJobs.mutate(null)} size="icon">
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
			</HeaderContainer>
			<div className="h-full mr-1 overflow-x-hidden custom-scroll inspector-scroll">
				<div className="">
					<div className="py-1">
						{runningJobs.data?.map((job) => (
							<Job key={job.id} job={job} />
						))}
						{jobs.data?.map((job) => (
							<Job key={job.id} job={job} />
						))}
						{jobs.data?.length === 0 && runningJobs.data?.length === 0 && (
							<div className="flex items-center justify-center h-32 text-ink-dull">No jobs.</div>
						)}
					</div>
				</div>
			</div>
		</div>
	);
}

function Job({ job }: { job: JobReport }) {
	const niceData = getNiceData(job)[job.name] || {
		name: job.name,
		icon: Question
	};
	const isRunning = job.status === 'Running';
	return (
		<div className="flex items-center px-2 py-2 pl-4 border-b border-app-line/50 bg-opacity-60">
			<Tooltip label={job.status}>
				<niceData.icon className={clsx('w-5 h-5 mr-3')} />
			</Tooltip>
			<div className="flex flex-col truncate">
				<span className="mt-0.5 font-semibold truncate">
					{isRunning ? job.message : niceData.name}
				</span>
				{isRunning && (
					<div className="w-full my-1">
						<ProgressBar value={job.completed_task_count} total={job.task_count} />
					</div>
				)}
				<div className="flex items-center truncate text-ink-faint">
					<span className="text-xs">
						{isRunning ? 'Elapsed' : job.status === 'Failed' ? 'Failed after' : 'Took'}{' '}
						{job.seconds_elapsed
							? dayjs.duration({ seconds: job.seconds_elapsed }).humanize()
							: 'less than a second'}
					</span>
					<span className="mx-1 opacity-50">&#8226;</span>
					{
						<span className="text-xs">
							{isRunning ? 'Unknown time remaining' : dayjs(job.date_created).toNow(true) + ' ago'}
						</span>
					}
				</div>
				{/* <span className="mt-0.5 opacity-50 text-tiny text-ink-faint">{job.id}</span> */}
			</div>
			<div className="flex-grow" />
			<div className="flex flex-row space-x-2 ml-7">
				{job.status === 'Running' && (
					<Button size="icon">
						<Tooltip label="Pause">
							<Pause className="w-4 h-4" />
						</Tooltip>
					</Button>
				)}
				{job.status === 'Failed' && (
					<Button size="icon">
						<Tooltip label="Retry">
							<ArrowsClockwise className="w-4" />
						</Tooltip>
					</Button>
				)}
				<Button size="icon">
					<Tooltip label="Remove">
						<X className="w-4 h-4" />
					</Tooltip>
				</Button>
			</div>
		</div>
	);
}

function numberWithCommas(x: number) {
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}
