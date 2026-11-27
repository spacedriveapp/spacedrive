import {Export, FilmReel} from '@phosphor-icons/react';
import type {
	Category,
	File,
	FootageStatusOutput,
	ListProvidersOutput,
	MediaSidecarsOutput
} from '@sd/ts-client';
import {getContentKind, useFootageJobStore} from '@sd/ts-client';
import clsx from 'clsx';
import {useEffect, useMemo} from 'react';
import {useServer} from '../../contexts/ServerContext';
import {useLibraryQuery} from '../../contexts/SpacedriveContext';
import {useOptionalExplorer} from '../../routes/explorer';
import {InfoRow, Section} from '../Inspector/primitives';
import {JobProgressBar} from '../JobManager/components/JobProgressBar';
import {useOptionalJobsContext} from '../JobManager/hooks/JobsContext';
import {
	FOOTAGE_JOBS,
	footageJobLabel,
	footagePathKey,
	formatTimecode,
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
	// The same query Quick Preview makes, so opening it from here is instant.
	const sidecars = useLibraryQuery(
		{type: 'media.sidecars', input: {path: file.sd_path}},
		{enabled: isVideo && file.is_local}
	);
	const providers = useLibraryQuery(
		{type: 'ai.providers.list', input: {}},
		{enabled: isVideo, staleTime: 30_000}
	).data;
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

	// A finished job has written sidecars, so what this section shows is stale.
	const refetchStatus = status.refetch;
	const refetchSidecars = sidecars.refetch;
	useEffect(() => {
		if (finished > 0 && isVideo) {
			void refetchStatus();
			void refetchSidecars();
		}
	}, [finished, isVideo, refetchStatus, refetchSidecars]);

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
			{isVideo && sidecars.data && <ShotStrip file={file} sidecars={sidecars.data} />}
			{isVideo && sidecars.data?.transcript && <TranscriptExcerpt file={file} sidecars={sidecars.data} />}
			{isVideo && (
				<AnalysisRows
					providers={providers}
					shots={status.data?.shots ?? null}
					described={sidecars.data?.descriptions.length ?? 0}
					analyzed={status.data?.analyzed_shots ?? null}
				/>
			)}

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

const STRIP_DESCRIPTIONS = 4;
const EXCERPT_CHARS = 220;

/** Keyframes of the detected shots; a click opens Quick Preview at the shot. */
function ShotStrip({file, sidecars}: {file: File; sidecars: MediaSidecarsOutput}) {
	const {buildSidecarUrl} = useServer();
	const explorer = useOptionalExplorer();
	const shots = sidecars.shots?.shots ?? [];
	if (shots.length === 0) return null;
	const descriptions = new Map(
		sidecars.descriptions.map((entry) => [entry.shot_id, entry.description])
	);
	const open = (seconds: number) =>
		explorer?.openQuickPreview(file.id, {startSeconds: seconds});
	const described = shots.filter((shot) => descriptions.has(shot.id));

	return (
		<div className="space-y-1.5 pt-1">
			<div className="no-scrollbar -mx-1 flex gap-1.5 overflow-x-auto px-1 pb-0.5">
				{shots.map((shot) => {
					const description = descriptions.get(shot.id);
					const src = buildSidecarUrl(sidecars.content_uuid, 'keyframe', shot.id, 'jpg');
					return (
						<button
							key={shot.id}
							type="button"
							onClick={() => open(shot.in.seconds)}
							disabled={!explorer}
							title={[`Shot ${shot.index + 1} at ${formatTimecode(shot.in.seconds)}`, description]
								.filter(Boolean)
								.join(': ')}
							className="group relative shrink-0 overflow-hidden rounded-md bg-sidebar-box/40"
						>
							{src && (
								<img
									src={src}
									alt=""
									loading="lazy"
									className="h-11 w-[78px] object-cover transition-opacity group-hover:opacity-80"
								/>
							)}
							<span className="absolute bottom-0.5 right-0.5 rounded bg-black/60 px-1 text-[9px] font-medium tabular-nums leading-tight text-white">
								{formatTimecode(shot.in.seconds)}
							</span>
						</button>
					);
				})}
			</div>
			{described.slice(0, STRIP_DESCRIPTIONS).map((shot) => (
				<button
					key={shot.id}
					type="button"
					onClick={() => open(shot.in.seconds)}
					disabled={!explorer}
					className="flex w-full gap-2 rounded text-left text-xs hover:bg-sidebar-box/30"
				>
					<span className="text-sidebar-inkDull shrink-0 tabular-nums">
						{formatTimecode(shot.in.seconds)}
					</span>
					<span className="text-sidebar-ink line-clamp-2">{descriptions.get(shot.id)}</span>
				</button>
			))}
			{described.length > STRIP_DESCRIPTIONS && (
				<p className="text-sidebar-inkDull text-[11px]">
					{described.length - STRIP_DESCRIPTIONS} more described shots
				</p>
			)}
		</div>
	);
}

/** The opening lines of the transcript, and a way into the full one. */
function TranscriptExcerpt({file, sidecars}: {file: File; sidecars: MediaSidecarsOutput}) {
	const explorer = useOptionalExplorer();
	const segments = sidecars.transcript?.segments ?? [];
	let excerpt = '';
	for (const segment of segments) {
		if (excerpt.length >= EXCERPT_CHARS) break;
		excerpt = `${excerpt} ${segment.text.trim()}`.trim();
	}
	if (!excerpt) return null;
	return (
		<div className="space-y-1 pt-1">
			<p className="text-sidebar-ink line-clamp-3 text-xs leading-snug">
				{excerpt.length > EXCERPT_CHARS ? `${excerpt.slice(0, EXCERPT_CHARS).trimEnd()}…` : excerpt}
			</p>
			{explorer && (
				<button
					type="button"
					onClick={() => explorer.openQuickPreview(file.id)}
					className="text-accent text-xs font-medium hover:underline"
				>
					Open transcript
				</button>
			)}
		</div>
	);
}

const ANALYSIS_CATEGORIES: {category: Category; label: string}[] = [
	{category: 'image_caption', label: 'Captions'},
	{category: 'scene_classification', label: 'Scene labels'},
	{category: 'face_detection', label: 'Faces'}
];

/**
 * What the Videos extension's analysis can do here, per capability, from
 * the same `ai.providers.list` answer Settings > Models shows. The extension
 * asks for local models only, so a cloud captioner does not count for it.
 */
function AnalysisRows({
	providers,
	shots,
	described,
	analyzed
}: {
	providers: ListProvidersOutput | undefined;
	shots: number | null;
	described: number;
	analyzed: number | null;
}) {
	if (!providers) return null;
	const missing: string[] = [];
	const rows = ANALYSIS_CATEGORIES.map(({category, label}) => {
		const found = providers.active.find((entry) => entry.category === category);
		const active = found?.locality === 'local' ? found : undefined;
		if (!active) missing.push(label.toLowerCase());
		const provider = active ? 'Local model' : found ? 'Cloud only' : 'No model';
		const progress =
			category === 'image_caption' && analyzed != null
				? shots != null
					? `${described} of ${shots} shots`
					: `${described} shots`
				: null;
		return {label, value: progress && active ? `${progress} · ${provider}` : provider};
	});
	return (
		<>
			{rows.map((row) => (
				<InfoRow key={row.label} label={row.label} value={row.value} />
			))}
			{missing.length > 0 && (
				<p className="text-sidebar-inkDull text-[11px] leading-snug">
					Analyze skips {missing.join(', ')} until a local model is installed in Settings › Models.
				</p>
			)}
		</>
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
