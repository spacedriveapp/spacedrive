import clsx from 'clsx';
import dayjs from 'dayjs';
import {
	ArrowsClockwise,
	Camera,
	Copy,
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
import { memo, useEffect, useState } from 'react';
import { JobReport } from '@sd/client';
import { Button, ProgressBar, Tooltip } from '@sd/ui';

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
		name: `Extracted metadata for ${numberWithCommas(
			job.metadata?.total_orphan_paths || 0
		)} files`,
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

function Job({ job, clearAJob }: { job: JobReport; clearAJob?: (arg: string) => void }) {
	const niceData = getNiceData(job)[job.name] || {
		name: job.name,
		icon: Question
	};
	const isRunning = job.status === 'Running';

	return (
		// Do we actually need bg-opacity-60 here? Where is the bg?
		// eslint-disable-next-line tailwindcss/migration-from-tailwind-2
		<div className="border-b border-app-line/50 p-3 pl-4">
			<div className="flex items-center bg-opacity-60">
				<Tooltip label={job.status}>
					<niceData.icon className={clsx('mr-3 h-5 w-5')} />
				</Tooltip>
				<div className="flex flex-col truncate">
					<span className="truncate font-semibold">{niceData.name}</span>
					<div className="flex items-center truncate text-ink-faint">
						<span className="text-xs">
							<JobTimeText job={job} />
						</span>
						{<span className="text-xs">{dayjs(job.created_at).fromNow()}</span>}
					</div>
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
					{job.status !== 'Running' && (
						<Button onClick={() => clearAJob?.(job.id)} size="icon">
							<Tooltip label="Remove">
								<X className="h-4 w-4" />
							</Tooltip>
						</Button>
					)}
				</div>
			</div>
			{isRunning && (
				<div className="mt-3 mb-1 w-full">
					<ProgressBar value={job.completed_task_count} total={job.task_count} />
				</div>
			)}
		</div>
	);
}

function JobTimeText({ job }: { job: JobReport }) {
	const [_, setRerenderPlz] = useState(0);

	let text: string;
	if (job.status === 'Running') {
		text = `Elapsed ${dayjs(job.started_at).fromNow(true)}`;
	} else if (job.completed_at) {
		text = `Took ${dayjs(job.started_at).from(job.completed_at, true)}`;
	} else {
		text = `Took ${dayjs(job.started_at).fromNow(true)}`;
	}

	const checkForNaN = text.split(' ').some((x) => isNaN(Number(x)));

	useEffect(() => {
		if (job.status === 'Running') {
			const interval = setInterval(() => {
				setRerenderPlz((x) => x + 1); // Trigger React to rerender and dayjs to update
			}, 1000);
			return () => clearInterval(interval);
		}
	}, [job.status]);

	return <>{checkForNaN ? '' : text}</>;
}

function numberWithCommas(x: number) {
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

export default memo(Job);
