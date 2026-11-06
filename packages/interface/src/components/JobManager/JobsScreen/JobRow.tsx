import {Pause, Play, X} from '@phosphor-icons/react';
import clsx from 'clsx';
import {useState} from 'react';
import {JobProgressSummary} from '../components/JobProgressSummary';
import {JobStatusIndicator} from '../components/JobStatusIndicator';
import type {JobListItem} from '../types';
import {formatDuration, getJobDisplayName, timeAgo} from '../types';

interface JobRowProps {
	job: JobListItem;
	onPause?: (jobId: string) => void;
	onResume?: (jobId: string) => void;
	onCancel?: (jobId: string) => void;
}

export function JobRow({job, onPause, onResume, onCancel}: JobRowProps) {
	const [isHovered, setIsHovered] = useState(false);

	const displayName = getJobDisplayName(job);
	const showActionButton =
		job.status === 'running' || job.status === 'paused';
	const canPause = job.status === 'running' && onPause;
	const canResume = job.status === 'paused' && onResume;
	const canCancel =
		(job.status === 'running' || job.status === 'paused') && onCancel;

	const handleAction = (e: React.MouseEvent) => {
		e.stopPropagation();
		if (canPause) {
			onPause(job.id);
		} else if (canResume) {
			onResume(job.id);
		}
	};

	const handleCancel = (e: React.MouseEvent) => {
		e.stopPropagation();
		if (canCancel) {
			onCancel(job.id);
		}
	};

	// Get phase and message
	const phase = job.current_phase;
	const message = job.status_message;

	// Calculate duration - prefer started_at for accuracy, fallback to created_at
	const startTime = job.started_at || job.created_at;
	const duration = startTime
		? job.completed_at
			? new Date(job.completed_at).getTime() -
				new Date(startTime).getTime()
			: Date.now() - new Date(startTime).getTime()
		: 0;

	return (
		<div
			className={clsx(
				'border-app-line/30 group relative flex items-center gap-4 border-b px-4 py-3',
				'hover:bg-app-hover/20'
			)}
			onMouseEnter={() => setIsHovered(true)}
			onMouseLeave={() => setIsHovered(false)}
		>
			{/* Icon */}
			<div className="flex-shrink-0">
				<JobStatusIndicator job={job} />
			</div>

			{/* Main info */}
			<div className="flex min-w-0 flex-1 items-center gap-6">
				{/* Job name and details */}
				<div className="min-w-0 flex-1">
					<div className="mb-1 flex items-center gap-2">
						<h3 className="text-ink mt-1 truncate text-sm font-medium">
							{displayName}
						</h3>
						{phase && (
							<span className="text-ink-dull bg-app-box rounded-full px-2 py-0.5 text-xs">
								{phase}
							</span>
						)}
					</div>
					{message && (
						<p className="text-ink-dull truncate text-xs">
							{message}
						</p>
					)}
				</div>

				{/* Progress / Duration column */}
				<div className="w-44 flex-shrink-0">
					{job.status === 'running' || job.status === 'paused' ? (
						<JobProgressSummary job={job} />
					) : job.status === 'completed' ? (
						// Show duration for completed jobs
						<span className="text-ink-dull text-xs">
							{formatDuration(duration)}
						</span>
					) : job.status === 'queued' ? (
						// Show waiting status for queued jobs
						<span className="text-ink-dull text-xs">
							Waiting...
						</span>
					) : (
						// Show dash for failed/cancelled jobs
						<span className="text-ink-dull text-xs">—</span>
					)}
				</div>

				{/* Completed/Started time */}
				<div className="w-24 flex-shrink-0 text-right">
					<span className="text-ink-dull text-xs">
						{job.status === 'completed' && job.completed_at
							? timeAgo(job.completed_at)
							: job.status === 'running' && job.started_at
								? timeAgo(job.started_at)
								: job.created_at
									? timeAgo(job.created_at)
									: '—'}
					</span>
				</div>

				{/* Status */}
				<div className="w-20 flex-shrink-0 text-right">
					<span
						className={clsx(
							'inline-flex items-center rounded-md px-2 py-1 text-xs font-medium',
							job.status === 'running' &&
								'bg-accent/10 text-accent',
							job.status === 'completed' &&
								'bg-app-line/30 text-ink-dull',
							job.status === 'failed' &&
								'bg-red-500/10 text-red-500',
							job.status === 'paused' &&
								'bg-yellow-500/10 text-yellow-500',
							job.status === 'queued' &&
								'bg-app-line/20 text-ink-dull'
						)}
					>
						{job.status}
					</span>
				</div>
			</div>

			{/* Action buttons */}
			{isHovered && (
				<div className="flex items-center gap-1">
					{showActionButton && (canPause || canResume) && (
						<button
							onClick={handleAction}
							className="bg-app-box hover:bg-app-selected flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full transition-colors"
							title={canPause ? 'Pause job' : 'Resume job'}
						>
							{canPause ? (
								<Pause
									size={12}
									weight="fill"
									className="text-ink"
								/>
							) : (
								<Play
									size={12}
									weight="fill"
									className="text-ink"
								/>
							)}
						</button>
					)}
					{canCancel && (
						<button
							onClick={handleCancel}
							className="bg-app-box flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full transition-colors hover:bg-red-500"
							title="Cancel job"
						>
							<X
								size={12}
								weight="bold"
								className="text-ink hover:text-white"
							/>
						</button>
					)}
				</div>
			)}
		</div>
	);
}
