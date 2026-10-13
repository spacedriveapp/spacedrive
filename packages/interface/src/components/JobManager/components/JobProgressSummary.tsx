import {useEffect, useState} from 'react';
import type {JobListItem} from '../types';
import {formatDuration} from '../types';

export function JobProgressSummary({
	job,
	compact = false
}: {
	job: JobListItem;
	compact?: boolean;
}) {
	const [now, setNow] = useState(Date.now());
	useEffect(() => {
		if (job.status !== 'running') return;
		const timer = setInterval(() => setNow(Date.now()), 1000);
		return () => clearInterval(timer);
	}, [job.id, job.status]);

	const generic = job.generic_progress;
	const [receivedAt, setReceivedAt] = useState(Date.now());
	useEffect(() => setReceivedAt(Date.now()), [job.id, generic]);

	const completed = generic?.completion.completed ?? 0;
	const total = generic?.completion.total ?? 0;
	const eta =
		now - receivedAt <= 30000
			? generic?.performance.estimated_remaining
			: null;
	const rate = generic?.performance.rate ?? 0;
	const start = job.started_at ?? job.created_at;
	const clockElapsed = start
		? Math.max(
				0,
				(job.completed_at ? Date.parse(job.completed_at) : now) -
					Date.parse(start)
			)
		: 0;
	const reportedElapsed = generic?.performance.elapsed;
	const sinceReport =
		job.status === 'running' ? Math.max(0, now - receivedAt) : 0;
	const elapsed = reportedElapsed
		? reportedElapsed.secs * 1000 +
			reportedElapsed.nanos / 1e6 +
			sinceReport
		: clockElapsed;
	const percent =
		job.status === 'completed'
			? 100
			: Math.min(99, Math.max(0, Math.floor(job.progress * 100)));
	const countFormat = new Intl.NumberFormat(
		undefined,
		compact ? {notation: 'compact', maximumFractionDigits: 1} : undefined
	);
	let text = `${percent}%`;
	if (total > 0) {
		text = `${countFormat.format(completed)} / ${countFormat.format(total)}`;
		if (!compact) text += ' processed';
	} else if (generic && completed > 0) {
		text = `${countFormat.format(completed)} processed`;
	} else if (generic?.phase) {
		text = generic.phase;
	}

	return (
		<div
			className="text-ink-dull min-w-0 text-xs"
			title={generic?.message ?? undefined}
		>
			<div className="truncate">{text}</div>
			{job.status === 'running' && (
				<div className="truncate">
					{eta
						? `About ${formatDuration(eta.secs * 1000)} left`
						: 'Estimating…'}
				</div>
			)}
			<div className="truncate">
				{formatDuration(elapsed)} elapsed
				{!compact && rate > 0
					? ` · ${Math.round(rate).toLocaleString()} items/s`
					: ''}
			</div>
		</div>
	);
}
