import {Export, FilmReel} from '@phosphor-icons/react';
import type {File, FootageStatusOutput} from '@sd/ts-client';
import {getContentKind} from '@sd/ts-client';
import clsx from 'clsx';
import {useEffect, useMemo} from 'react';
import {useLibraryQuery} from '../../contexts/SpacedriveContext';
import {InfoRow, Section} from '../Inspector/primitives';
import {JobProgressBar} from '../JobManager/components/JobProgressBar';
import {useOptionalJobsContext} from '../JobManager/hooks/JobsContext';
import {useFootageJobStore} from './footageJobStore';
import {
	FOOTAGE_JOBS,
	footageJobLabel,
	footagePathKey,
	isFootageTarget,
	useFootageActions,
	useFootageCapabilities
} from './useFootage';

const ACTIVE = new Set(['queued', 'running', 'paused']);

/**
 * Footage work for one video or folder in the inspector: what its bytes
 * already have, the jobs a person can start, and the progress of the ones
 * started from the app. Nothing here runs until someone asks.
 */
export function FootageSection({file}: {file: File}) {
	if (!isFootageTarget(file)) return null;
	return <FootageSectionContent file={file} />;
}

function FootageSectionContent({file}: {file: File}) {
	const isVideo = getContentKind(file) === 'video';
	const capabilities = useFootageCapabilities().data;
	const status = useLibraryQuery(
		{type: 'media.footage.status', input: {path: file.sd_path}},
		{enabled: isVideo}
	);
	const {start, startExport, canPickFolder, isBusy} = useFootageActions();
	const started = useFootageJobStore((state) => state.byPath[footagePathKey(file.sd_path)]);
	const jobsContext = useOptionalJobsContext();

	const tracked = useMemo(
		() =>
			(started ?? []).flatMap((entry) => {
				const job = jobsContext?.jobs.find((candidate) => candidate.id === entry.jobId);
				return job ? [{...entry, item: job}] : [];
			}),
		[started, jobsContext?.jobs]
	);
	const active = tracked.filter((entry) => ACTIVE.has(entry.item.status));
	const finished = tracked.length - active.length;

	// A finished job has written sidecars, so the status above it is stale.
	const refetchStatus = status.refetch;
	useEffect(() => {
		if (finished > 0 && isVideo) void refetchStatus();
	}, [finished, isVideo, refetchStatus]);

	const blocked = capabilities
		? [...new Set(
				[...FOOTAGE_JOBS.map(({job}) => capabilities[job]), capabilities.export]
					.map((support) => support.reason)
					.filter((reason): reason is string => !!reason)
			)]
		: [];

	return (
		<Section title="Footage" icon={FilmReel}>
			{isVideo && <FootageStatusRows status={status.data} loading={status.isLoading} error={status.error} />}

			{active.map((entry) => (
				<div key={entry.jobId} className="text-xs">
					<div className="flex justify-between gap-3">
						<span className="text-sidebar-ink font-medium">{footageJobLabel(entry.job)}</span>
						<span className="text-sidebar-inkDull tabular-nums">
							{Math.round(entry.item.progress * 100)}%
						</span>
					</div>
					<JobProgressBar progress={entry.item.progress} status={entry.item.status} />
				</div>
			))}

			<div className="grid grid-cols-2 gap-1.5 pt-1">
				{FOOTAGE_JOBS.map(({job, label, icon: Icon}) => {
					const support = capabilities?.[job];
					const running = active.some((entry) => entry.job === job);
					return (
						<FootageButton
							key={job}
							label={running ? `${label}…` : label}
							icon={Icon}
							disabled={!support?.available || running || isBusy}
							title={support?.reason ?? undefined}
							onClick={() => void start([file], job)}
						/>
					);
				})}
				<FootageButton
					label="Export timeline"
					icon={Export}
					className="col-span-2"
					disabled={!capabilities?.export.available || !canPickFolder || isBusy}
					title={capabilities?.export.reason ?? 'FCPXML, EDL and OTIO, cut at detected shots'}
					onClick={() => void startExport([file])}
				/>
			</div>

			{blocked.map((reason) => (
				<p key={reason} className="text-sidebar-inkDull text-[11px] leading-snug">
					{reason}
				</p>
			))}
		</Section>
	);
}

function FootageStatusRows({
	status,
	loading,
	error
}: {
	status: FootageStatusOutput | undefined;
	loading: boolean;
	error: unknown;
}) {
	if (error) {
		return <p className="text-sidebar-inkDull text-xs">{String(error)}</p>;
	}
	if (!status) {
		return loading ? <p className="text-sidebar-inkDull text-xs">Reading footage state…</p> : null;
	}
	const analysis =
		status.analyzed_shots == null
			? 'Not analyzed'
			: status.shots != null
				? `${status.analyzed_shots} of ${status.shots} shots`
				: `${status.analyzed_shots} shots`;
	return (
		<>
			<InfoRow label="Transcript" value={status.transcript ? 'Yes' : 'None'} />
			<InfoRow label="Proxy" value={status.proxy ? 'Yes' : 'None'} />
			<InfoRow label="Shots" value={status.shots == null ? 'Not detected' : status.shots} />
			<InfoRow label="Analysis" value={analysis} />
		</>
	);
}

function FootageButton({
	label,
	icon: Icon,
	disabled,
	title,
	className,
	onClick
}: {
	label: string;
	icon: typeof Export;
	disabled: boolean;
	title?: string;
	className?: string;
	onClick: () => void;
}) {
	return (
		<button
			type="button"
			onClick={onClick}
			disabled={disabled}
			title={title}
			className={clsx(
				'border-sidebar-line/30 bg-sidebar-box/20 text-sidebar-ink flex items-center justify-center gap-1.5 rounded-md border px-2 py-1.5 text-xs font-medium transition-colors',
				disabled ? 'cursor-default opacity-50' : 'hover:bg-sidebar-box/40 active:scale-[0.98]',
				className
			)}
		>
			<Icon size={13} weight="bold" />
			<span className="truncate">{label}</span>
		</button>
	);
}
