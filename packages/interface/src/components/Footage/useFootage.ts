import type {Icon} from '@phosphor-icons/react';
import {FilmSlate, FilmStrip, MagicWand, Subtitles} from '@phosphor-icons/react';
import type {
	File,
	FootageJob,
	FootageSummariesInput,
	FootageSummariesOutput,
	FootageSummary,
	SdPath
} from '@sd/ts-client';
import {getContentKind, isVirtualFile, useFootageJobStore, WIRE_METHODS} from '@sd/ts-client';
import {useQuery} from '@tanstack/react-query';
import {toast} from '@spacedrive/primitives';
import {useMemo} from 'react';
import {usePlatform} from '../../contexts/PlatformContext';
import {
	useLibraryMutation,
	useLibraryQuery,
	useSpacedriveClient
} from '../../contexts/SpacedriveContext';

export const FOOTAGE_JOBS: {job: FootageJob; label: string; icon: Icon}[] = [
	{job: 'transcribe', label: 'Transcribe', icon: Subtitles},
	{job: 'proxy', label: 'Make proxy', icon: FilmSlate},
	{job: 'shots', label: 'Detect shots', icon: FilmStrip},
	{job: 'analyze', label: 'Analyze', icon: MagicWand}
];

export function footageJobLabel(job: FootageJob) {
	return FOOTAGE_JOBS.find((entry) => entry.job === job)?.label ?? job;
}

export function footagePathKey(path: SdPath) {
	return 'Physical' in path ? `${path.Physical.device_slug}:${path.Physical.path}` : '';
}

/** A video or folder on this device, which is all footage work can read. */
export function isFootageTarget(file: File) {
	return (
		file.is_local &&
		'Physical' in file.sd_path &&
		!isVirtualFile(file) &&
		(file.kind === 'Directory' || getContentKind(file) === 'video')
	);
}

export function useFootageCapabilities() {
	return useLibraryQuery(
		{type: 'media.footage.capabilities', input: {}},
		{staleTime: 30_000}
	);
}

/** Starts footage jobs and timeline exports, with a toast for each outcome. */
export function useFootageActions() {
	const platform = usePlatform();
	const run = useLibraryMutation('media.footage.run');
	const exportTimeline = useLibraryMutation('media.footage.export');
	const track = useFootageJobStore((state) => state.track);

	const start = async (targets: File[], job: FootageJob) => {
		const label = footageJobLabel(job);
		let started = 0;
		for (const target of targets) {
			try {
				const {job_ids} = await run.mutateAsync({path: target.sd_path, job});
				track(footagePathKey(target.sd_path), job, job_ids);
				started += job_ids.length;
			} catch (error) {
				toast.error({title: `${label} could not start`, body: String(error)});
				return;
			}
		}
		if (started === 0) {
			toast.info({title: label, body: 'There are no videos to process here'});
			return;
		}
		toast.success({
			title: `${label} started`,
			body: started === 1 ? 'Follow it in Jobs' : `${started} jobs, follow them in Jobs`
		});
	};

	const startExport = async (targets: File[]) => {
		const first = targets[0];
		if (!first || !('Physical' in first.sd_path) || !platform.openDirectoryPickerDialog) return;
		const picked = await platform.openDirectoryPickerDialog({
			title: 'Choose where to save the timeline',
			multiple: false
		});
		if (!picked || typeof picked !== 'string') return;
		try {
			const result = await exportTimeline.mutateAsync({
				paths: targets.map((target) => target.sd_path),
				destination: {
					Physical: {device_slug: first.sd_path.Physical.device_slug, path: picked}
				},
				name: targets.length === 1 ? first.name : 'Spacedrive timeline',
				formats: [],
				media: 'original'
			});
			const names = result.files
				.map((path) => ('Physical' in path ? path.Physical.path.split('/').pop() : ''))
				.join(', ');
			toast.success({
				title: `Exported ${result.clips} clip${result.clips === 1 ? '' : 's'}`,
				body: [names, ...result.warnings].filter(Boolean).join('\n')
			});
		} catch (error) {
			toast.error({title: 'Timeline export failed', body: String(error)});
		}
	};

	return {
		start,
		startExport,
		canPickFolder: !!platform.openDirectoryPickerDialog,
		isBusy: run.isPending || exportTimeline.isPending
	};
}

/** A video's length in seconds, from its media data or what shot detection measured. */
export function videoDuration(file: File, footage?: FootageSummary): number | null {
	return (
		file.video_media_data?.duration_seconds ??
		file.duration_seconds ??
		footage?.duration_seconds ??
		null
	);
}

/** `1:05` or `1:02:05`, the way a player shows a position. */
export function formatTimecode(seconds: number): string {
	const total = Math.max(0, Math.floor(seconds));
	const h = Math.floor(total / 3600);
	const m = Math.floor(total / 60) % 60;
	const s = String(total % 60).padStart(2, '0');
	return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
}

/** A local video's key in a summaries map, or null for anything else. */
export function footageKey(file: File): string | null {
	if (!file.is_local || getContentKind(file) !== 'video') return null;
	return footagePathKey(file.sd_path) || null;
}

// The daemon answers at most this many paths per query.
const SUMMARIES_PER_QUERY = 5000;
const NO_SUMMARIES = new Map<string, FootageSummary>();

/** The query key every footage summary read starts with, so one invalidation
 * refreshes them all after a footage job writes sidecars. */
export const FOOTAGE_SUMMARIES_KEY = 'media.footage.summaries';

type Client = ReturnType<typeof useSpacedriveClient>;

/**
 * Throws when the selected library is no longer the one a read started in.
 * The client sends every request to the current library, so a read that
 * spans several requests would otherwise mix two libraries' answers and
 * cache them under the first one's key.
 */
export function assertLibrary(client: Client, libraryId: string) {
	if (client.getCurrentLibraryId() !== libraryId) {
		throw new Error('The library changed while footage was being read.');
	}
}

/** Footage summaries for any number of videos, in batches the daemon accepts. */
export async function fetchFootageSummaries(
	client: Client,
	libraryId: string,
	paths: SdPath[]
): Promise<Map<string, FootageSummary>> {
	const summaries = new Map<string, FootageSummary>();
	for (let start = 0; start < paths.length; start += SUMMARIES_PER_QUERY) {
		assertLibrary(client, libraryId);
		const output = await client.execute<FootageSummariesInput, FootageSummariesOutput>(
			WIRE_METHODS.libraryQueries[FOOTAGE_SUMMARIES_KEY],
			{paths: paths.slice(start, start + SUMMARIES_PER_QUERY)}
		);
		assertLibrary(client, libraryId);
		for (const summary of output.summaries) {
			summaries.set(footagePathKey(summary.path), summary);
		}
	}
	return summaries;
}

/** The distinct paths of the local videos among files. */
export function footagePaths(files: File[]): SdPath[] {
	const seen = new Set<string>();
	const paths: SdPath[] = [];
	for (const file of files) {
		const key = footageKey(file);
		if (!key || seen.has(key)) continue;
		seen.add(key);
		paths.push(file.sd_path);
	}
	return paths;
}

/**
 * The footage state of every local video in a listing, keyed by
 * `footageKey`, from one query for the whole listing. A row reads its entry
 * from the map instead of asking on its own, and a video missing from the
 * map has had nothing made for it.
 */
export function useFootageSummaries(files: File[], enabled: boolean) {
	const client = useSpacedriveClient();
	const libraryId = client.getCurrentLibraryId();
	const paths = useMemo(() => (enabled ? footagePaths(files) : []), [files, enabled]);

	const {data, isLoading} = useQuery({
		queryKey: [FOOTAGE_SUMMARIES_KEY, libraryId, paths],
		queryFn: () => fetchFootageSummaries(client, libraryId!, paths),
		enabled: !!libraryId && paths.length > 0,
		staleTime: 30_000
	});
	return {summaries: data ?? NO_SUMMARIES, isLoading: paths.length > 0 && isLoading};
}
