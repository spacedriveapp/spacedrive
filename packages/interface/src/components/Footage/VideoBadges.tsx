import {FilmStrip, Subtitles} from '@phosphor-icons/react';
import type {File, FootageSummary} from '@sd/ts-client';
import clsx from 'clsx';
import {formatTimecode, videoDuration} from './useFootage';

interface VideoDetailsShown {
	duration: boolean;
	shots: boolean;
	transcript: boolean;
}

/** Duration, shot count and transcript marker over a grid thumbnail. */
export function VideoBadges({
	file,
	footage,
	show,
	className
}: {
	file: File;
	footage: FootageSummary | undefined;
	show: VideoDetailsShown;
	className?: string;
}) {
	const duration = show.duration ? videoDuration(file, footage) : null;
	const shots = show.shots ? footage?.shots : null;
	const transcript = show.transcript && footage?.transcript;
	if (duration == null && shots == null && !transcript) return null;
	return (
		<div
			className={clsx(
				'pointer-events-none flex items-center gap-1 text-[10px] font-medium leading-none text-white',
				className
			)}
		>
			{transcript && (
				<span className="flex items-center rounded bg-black/60 p-[3px]" title="Has a transcript">
					<Subtitles size={10} weight="bold" />
				</span>
			)}
			{shots != null && (
				<span className="flex items-center gap-0.5 rounded bg-black/60 px-1 py-[3px] tabular-nums" title={`${shots} shots`}>
					<FilmStrip size={10} weight="bold" />
					{shots}
				</span>
			)}
			{duration != null && (
				<span className="rounded bg-black/60 px-1 py-[3px] tabular-nums">{formatTimecode(duration)}</span>
			)}
		</div>
	);
}
