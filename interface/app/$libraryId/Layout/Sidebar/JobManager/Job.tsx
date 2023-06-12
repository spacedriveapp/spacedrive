import clsx from 'clsx';
import dayjs from 'dayjs';
import {
	Camera,
	Copy,
	Eye,
	Fingerprint,
	Folder,
	LockSimple,
	LockSimpleOpen,
	Question,
	Scissors,
	Trash,
	TrashSimple
} from 'phosphor-react';
import { memo } from 'react';
import { JobReport } from '@sd/client';
import { ProgressBar } from '@sd/ui';
import './Job.scss';

interface JobNiceData {
	name: string;
	icon: React.ForwardRefExoticComponent<any>;
	subtext: string;
}

const getNiceData = (
	job: JobReport,
	isGroup: boolean | undefined
): Record<string, JobNiceData> => ({
	indexer: {
		name: isGroup
			? 'Indexing paths'
			: job.metadata?.location_path
				? `Indexed paths at ${job.metadata?.location_path} `
				: `Processing added location...`,
		icon: Folder,
		subtext: `${numberWithCommas(job.metadata?.total_paths || 0)} ${appendPlural(job, 'path')}`
	},
	thumbnailer: {
		name: `${job.status === 'Running' || job.status === 'Queued'
			? 'Generating thumbnails'
			: 'Generated thumbnails'
			}`,
		icon: Camera,
		subtext: `${numberWithCommas(job.completed_task_count)} of ${numberWithCommas(
			job.task_count
		)} ${appendPlural(job, 'thumbnail')}`
	},
	shallow_thumbnailer: {
		name: `Generating thumbnails for current directory`,
		icon: Camera,
		subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job, 'item')}`
	},
	file_identifier: {
		name: `${job.status === 'Running' || job.status === 'Queued'
			? 'Extracting metadata'
			: 'Extracted metadata'
			}`,
		icon: Eye,
		subtext:
			job.message ||
			`${numberWithCommas(job.metadata?.total_orphan_paths)} ${appendPlural(
				job,
				'file',
				'file_identifier'
			)}`
	},
	object_validator: {
		name: `Generated full object hashes`,
		icon: Fingerprint,
		subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job, 'object')}`
	},
	file_encryptor: {
		name: `Encrypted`,
		icon: LockSimple,
		subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job, 'file')}`
	},
	file_decryptor: {
		name: `Decrypted`,
		icon: LockSimpleOpen,
		subtext: `${numberWithCommas(job.task_count)}${appendPlural(job, 'file')}`
	},
	file_eraser: {
		name: `Securely erased`,
		icon: TrashSimple,
		subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job, 'file')}`
	},
	file_deleter: {
		name: `Deleted`,
		icon: Trash,
		subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job, 'file')}`
	},
	file_copier: {
		name: `Copied`,
		icon: Copy,
		subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job, 'file')}`
	},
	file_cutter: {
		name: `Moved`,
		icon: Scissors,
		subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job, 'file')}`
	}
});

interface JobProps {
	job: JobReport;
	clearJob?: (arg: string) => void;
	className?: string;
	isGroup?: boolean;
}

function formatEstimatedRemainingTime(end_date: string) {
	const duration = dayjs.duration(new Date(end_date).getTime() - Date.now());

	if (duration.hours() > 0) {
		return `${duration.hours()} hour${duration.hours() > 1 ? 's' : ''} remaining`;
	} else if (duration.minutes() > 0) {
		return `${duration.minutes()} minute${duration.minutes() > 1 ? 's' : ''} remaining`;
	} else {
		return `${duration.seconds()} second${duration.seconds() > 1 ? 's' : ''} remaining`;
	}
}

function Job({ job, clearJob, className, isGroup }: JobProps) {
	const niceData = getNiceData(job, isGroup)[job.name] || {
		name: job.name,
		icon: Question,
		subtext: job.name
	};
	const isRunning = job.status === 'Running';

	// dayjs from seconds to time
	const time = isRunning ? formatEstimatedRemainingTime(job.estimated_completion) : '';

	return (
		<li
			className={clsx(
				`removelistdot border-b border-app-line/50 pl-4`,
				className,
				isGroup ? `joblistitem pr-3 pt-0` : 'p-3'
			)}
		>
			<div className="flex">
				<div>
					<niceData.icon
						className={clsx(
							isGroup && 'ml-9 mr-3.5',
							'relative top-2 z-20 mr-3 h-6 w-6 rounded-full bg-app-button p-[5.5px]'
						)}
					/>
				</div>
				<div className="flex w-full flex-col">
					<div className="flex items-center">
						<div className="truncate">
							<span className="truncate font-semibold">{niceData.name}</span>
							<p className="mb-[5px] mt-[2px] flex gap-1 truncate text-sidebar-inkFaint">
								{job.status === 'Queued' && <p>{job.status}:</p>}
								{niceData.subtext}
								{time && ' • '}
								<span className="truncate">{time}</span>
							</p>
							<div className="flex gap-1 truncate text-sidebar-inkFaint"></div>
						</div>
						<div className="grow" />
						<div className="ml-7 flex flex-row space-x-2">
							{/* {job.status === 'Running' && (
						<Button size="icon">
							<Tooltip label="Coming Soon">
								<Pause weight="fill" className="w-4 h-4 opacity-30" />
							</Tooltip>
						</Button>
					)}
					{job.status === 'Failed' && (
						<Button size="icon">
							<Tooltip label="Coming Soon">
								<ArrowsClockwise className="w-4 opacity-30" />
							</Tooltip>
						</Button>
					)} */}
						</div>
					</div>
					{isRunning && (
						<div className="my-1 w-[335px]">
							<ProgressBar value={job.completed_task_count} total={job.task_count} />
						</div>
					)}
				</div>
			</div>
		</li>
	);
}

function appendPlural(job: JobReport, word: string, niceDataKey?: string) {
	const condition = (condition: boolean) => (condition ? `${word}s` : `${word}`);
	switch (niceDataKey) {
		case 'file_identifier':
			return condition(job.metadata?.total_orphan_paths > 1);
		default:
			return condition(job.task_count > 1);
	}
}

function numberWithCommas(x: number) {
	if (!x) return 0;
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

export default memo(Job);
