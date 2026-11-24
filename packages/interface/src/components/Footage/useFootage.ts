import type {Icon} from '@phosphor-icons/react';
import {FilmSlate, FilmStrip, MagicWand, Subtitles} from '@phosphor-icons/react';
import type {File, FootageJob, SdPath} from '@sd/ts-client';
import {getContentKind, isVirtualFile} from '@sd/ts-client';
import {toast} from '@spacedrive/primitives';
import {usePlatform} from '../../contexts/PlatformContext';
import {useLibraryMutation, useLibraryQuery} from '../../contexts/SpacedriveContext';
import {useFootageJobStore} from './footageJobStore';

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
