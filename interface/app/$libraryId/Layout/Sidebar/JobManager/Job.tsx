import clsx from 'clsx';
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
import { useJobTimeText } from './useJobTimeText';

interface JobNiceData {
	name: string;
	icon: React.ForwardRefExoticComponent<any>;
	filesDiscovered: string;
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
		filesDiscovered: `${numberWithCommas(
			job.metadata?.total_paths || 0
		)} ${JobCountTextCondition(job, 'path')}`
	},
	thumbnailer: {
		name: `${
			job.status === 'Running' || job.status === 'Queued'
				? 'Generating thumbnails'
				: 'Generated thumbnails'
		}`,
		icon: Camera,
		filesDiscovered: `${numberWithCommas(job.task_count)} ${JobCountTextCondition(job, 'path')}`
	},
	file_identifier: {
		name: `${
			job.status === 'Running' || job.status === 'Queued'
				? 'Extracting metadata'
				: 'Extracted metadata'
		}`,
		icon: Eye,
		filesDiscovered:
			job.message ||
			`${numberWithCommas(job.task_count)} ${JobCountTextCondition(job, 'task')}`
	},
	object_validator: {
		name: `Generated full object hashes`,
		icon: Fingerprint,
		filesDiscovered: `${numberWithCommas(job.task_count)} ${JobCountTextCondition(
			job,
			'object'
		)}`
	},
	file_encryptor: {
		name: `Encrypted`,
		icon: LockSimple,
		filesDiscovered: `${numberWithCommas(job.task_count)} ${JobCountTextCondition(job, 'file')}`
	},
	file_decryptor: {
		name: `Decrypted`,
		icon: LockSimpleOpen,
		filesDiscovered: `${numberWithCommas(job.task_count)}${JobCountTextCondition(job, 'file')}`
	},
	file_eraser: {
		name: `Securely erased`,
		icon: TrashSimple,
		filesDiscovered: `${numberWithCommas(job.task_count)} ${JobCountTextCondition(job, 'file')}`
	},
	file_deleter: {
		name: `Deleted`,
		icon: Trash,
		filesDiscovered: `${numberWithCommas(job.task_count)} ${JobCountTextCondition(job, 'file')}`
	},
	file_copier: {
		name: `Copied`,
		icon: Copy,
		filesDiscovered: `${numberWithCommas(job.task_count)} ${JobCountTextCondition(job, 'file')}`
	},
	file_cutter: {
		name: `Moved`,
		icon: Scissors,
		filesDiscovered: `${numberWithCommas(job.task_count)} ${JobCountTextCondition(job, 'file')}`
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

interface JobProps {
	job: JobReport;
	clearJob?: (arg: string) => void;
	className?: string;
	isGroup?: boolean;
}

function Job({ job, clearJob, className, isGroup }: JobProps) {
	const niceData = getNiceData(job, isGroup)[job.name] || {
		name: job.name,
		icon: Question,
		filesDiscovered: job.name
	};
	const isRunning = job.status === 'Running';

	const time = useJobTimeText(job);

	return (
		<li
			className={clsx(
				`removelistdot border-b border-app-line/50 pl-4`,
				className,
				isGroup ? `joblistitem pr-3 pt-0` : 'p-3'
			)}
		>
			<div className="ml-7 flex">
				<div>
					<niceData.icon
						className={clsx(
							'relative top-2 mr-3 h-6 w-6 rounded-full bg-app-button p-[5.5px]'
						)}
					/>
				</div>
				<div className="flex w-full flex-col">
					<div className="flex items-center">
						<div className="truncate">
							<span className="truncate font-semibold">{niceData.name}</span>
							<p className="mb-[5px] mt-[2px] flex gap-1 truncate text-ink-faint">
								{job.status === 'Queued' && <p>{job.status}:</p>}
								{niceData.filesDiscovered}
								{time && ' • '}
								<span className="truncate">{time}</span>
							</p>
							<div className="flex gap-1 truncate text-ink-faint"></div>
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

function JobCountTextCondition(job: JobReport, word: string) {
	const addStoEnd = job.task_count > 1 || job?.task_count === 0 ? `${word}s` : `${word}`;
	return addStoEnd;
}

function numberWithCommas(x: number) {
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

export default memo(Job);
