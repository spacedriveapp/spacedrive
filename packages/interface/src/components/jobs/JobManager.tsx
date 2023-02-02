import clsx from 'clsx';
import dayjs from 'dayjs';
import {
	ArrowsClockwise,
	Camera,
	Copy,
	DotsThree,
	Eye,
	Fingerprint,
	Folder,
	LockSimple,
	LockSimpleOpen,
	Pause,
	Question,
	Scissors,
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
	},
	file_copier: {
		name: `Copied ${numberWithCommas(job.task_count)} ${
			job.task_count > 1 || job.task_count === 0 ? 'files' : 'file'
		}`,
		icon: Copy
	},
	file_cutter: {
		name: `Moved ${numberWithCommas(job.task_count)} ${
			job.task_count > 1 || job.task_count === 0 ? 'files' : 'file'
		}`,
		icon: Scissors
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
		<div className="h-full overflow-hidden pb-10">
			<HeaderContainer>
				<CategoryHeading className="ml-2">Recent Jobs</CategoryHeading>
				<div className="flex-grow" />

				<Button onClick={() => clearAllJobs.mutate(null)} size="icon">
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
			</HeaderContainer>
			<div className="custom-scroll inspector-scroll mr-1 h-full overflow-x-hidden">
				<div className="">
					<div className="py-1">
						{runningJobs.data?.map((job) => (
							<Job key={job.id} job={job} />
						))}
						{jobs.data?.map((job) => (
							<Job key={job.id} job={job} />
						))}
						{jobs.data?.length === 0 && runningJobs.data?.length === 0 && (
							<div className="text-ink-dull flex h-32 items-center justify-center">No jobs.</div>
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
		<div className="border-app-line/50 flex items-center border-b bg-opacity-60 px-2 py-2 pl-4">
			<Tooltip label={job.status}>
				<niceData.icon className={clsx('mr-3 h-5 w-5')} />
			</Tooltip>
			<div className="flex flex-col truncate">
				<span className="mt-0.5 truncate font-semibold">
					{isRunning ? job.message : niceData.name}
				</span>
				{isRunning && (
					<div className="my-1 w-full">
						<ProgressBar value={job.completed_task_count} total={job.task_count} />
					</div>
				)}
				<div className="text-ink-faint flex items-center truncate">
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
			<div className="ml-7 flex flex-row space-x-2">
				{job.status === 'Running' && (
					<Button size="icon">
						<Tooltip label="Pause">
							<Pause className="h-4 w-4" />
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
						<X className="h-4 w-4" />
					</Tooltip>
				</Button>
			</div>
		</div>
	);
}

function numberWithCommas(x: number) {
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}
