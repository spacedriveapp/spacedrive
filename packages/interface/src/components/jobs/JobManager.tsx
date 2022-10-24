import {
	EllipsisHorizontalIcon,
	EllipsisVerticalIcon,
	EyeIcon,
	FingerPrintIcon,
	FolderIcon,
	PhotoIcon,
	XMarkIcon
} from '@heroicons/react/24/solid';
import { QuestionMarkCircleIcon } from '@heroicons/react/24/solid';
import { useLibraryQuery } from '@sd/client';
import { JobReport } from '@sd/client';
import { Button, CategoryHeading, tw } from '@sd/ui';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { ArrowsClockwise, Pause } from 'phosphor-react';

import ProgressBar from '../primitive/ProgressBar';
import { Tooltip } from '../tooltip/Tooltip';

interface JobNiceData {
	name: string;
	icon: React.FC<React.ComponentProps<'svg'>>;
}

const getNiceData = (job: JobReport): Record<string, JobNiceData> => ({
	indexer: {
		name: `Indexed ${numberWithCommas(job.metadata?.data?.total_paths || 0)} paths at "${
			job.metadata?.data?.location_path || '?'
		}"`,
		icon: FolderIcon
	},
	thumbnailer: {
		name: `Generated ${numberWithCommas(job.task_count)} thumbnails`,
		icon: PhotoIcon
	},
	file_identifier: {
		name: `Extracted metadata for ${numberWithCommas(job.task_count)} files`,
		icon: EyeIcon
	},
	object_validator: {
		name: `Generated ${numberWithCommas(job.task_count)} full object hashes`,
		icon: FingerPrintIcon
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

const HeaderContainer = tw.div`z-20 flex items-center w-full h-10 px-2 border-b border-app-line rounded-t-md bg-app-selected`;

export function JobsManager() {
	const runningJobs = useLibraryQuery(['jobs.getRunning']);
	const jobs = useLibraryQuery(['jobs.getHistory']);

	return (
		<div className="h-full pb-10 overflow-hidden">
			<HeaderContainer>
				<CategoryHeading className="ml-2">Recent Jobs</CategoryHeading>
				<div className="flex-grow" />
				<Button size="icon">
					<EllipsisHorizontalIcon className="w-5" />
				</Button>
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
					</div>
				</div>
			</div>
		</div>
	);
}

function Job({ job }: { job: JobReport }) {
	const niceData = getNiceData(job)[job.name] || {
		name: job.name,
		icon: QuestionMarkCircleIcon
	};
	const isRunning = job.status === 'Running';
	return (
		<div className="flex items-center px-2 py-2 pl-4 border-b border-app-line/50 bg-opacity-60">
			<Tooltip label={job.status}>
				<niceData.icon className={clsx('w-5 mr-3')} />
			</Tooltip>
			<div className="flex flex-col w-full ">
				<span className="flex mt-0.5 items-center font-semibold truncate">
					{isRunning ? job.message : niceData.name}
				</span>
				{isRunning && (
					<div className="w-full my-1">
						<ProgressBar value={job.completed_task_count} total={job.task_count} />
					</div>
				)}
				<div className="flex items-center text-ink-dull">
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
				<span className="mt-0.5 opacity-50 text-tiny text-ink-faint">{job.id}</span>
			</div>
			<div className="flex-grow" />
			<div className="flex flex-row space-x-2 ml-7">
				{job.status === 'Running' && (
					<Button size="icon">
						<Pause className="w-4" />
					</Button>
				)}
				{job.status === 'Failed' && (
					<Button size="icon">
						<ArrowsClockwise className="w-4" />
					</Button>
				)}
				<Button size="icon">
					<XMarkIcon className="w-4" />
				</Button>
			</div>
		</div>
	);
}

function numberWithCommas(x: number) {
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}
