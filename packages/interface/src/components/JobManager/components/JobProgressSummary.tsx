import {useEffect, useState} from 'react';
import type {JobListItem} from '../types';
import {formatDuration} from '../types';

export function JobProgressSummary({job, compact = false}: {job: JobListItem; compact?: boolean}) {
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
	const eta = now - receivedAt <= 30000 ? generic?.performance.estimated_remaining : null;
	const rate = generic?.performance.rate ?? 0;
	const start = job.started_at ?? job.created_at;
	const elapsed = start ? Math.max(0, (job.completed_at ? Date.parse(job.completed_at) : now) - Date.parse(start)) : 0;
	const contentJob = job.name === 'content_identity';
	const percent = job.status === 'completed' ? 100 : Math.min(99, Math.max(0, Math.floor(job.progress * 100)));
	const compactCount = new Intl.NumberFormat(undefined, {notation: 'compact', maximumFractionDigits: 1});
	const text = compact && total > 0 ? `${compactCount.format(completed)} / ${compactCount.format(total)}` : total > 0 ? `${completed.toLocaleString()} / ${total.toLocaleString()} ${contentJob ? 'checked' : 'processed'}` : contentJob ? generic ? `${compact ? compactCount.format(completed) : completed.toLocaleString()} ${compact ? 'checked' : 'files checked'}` : 'Waiting for progress…' : `${percent}%`;
	return <div className="text-xs text-ink-dull min-w-0" title={generic?.message ?? undefined}>
		<div className="truncate">{text}</div>
		{job.status === 'running' && <div className="truncate">{eta ? `About ${formatDuration(eta.secs * 1000)} left` : 'Estimating…'}</div>}
		<div className="truncate">{formatDuration(elapsed)} elapsed{!compact && rate > 0 && contentJob ? ` · ${Math.round(rate).toLocaleString()} files/s` : ''}</div>
	</div>;
}
