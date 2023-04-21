import * as icons from '@sd/assets/icons';
import clsx from 'clsx';
import { useRef, useState } from 'react';
import { ExplorerItem, isKeyOf, useLibraryContext } from '@sd/client';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import { useIsDark, usePlatform } from '~/util/Platform';
import { getExplorerItemData } from '../util';
import classes from './Thumb.module.scss';

export const getIcon = (
	isDir: boolean,
	isDark: boolean,
	kind: string,
	extension?: string | null
) => {
	if (isDir) return icons[isDark ? 'Folder' : 'Folder_Light'];

	let document: Extract<keyof typeof icons, 'Document' | 'Document_Light'> = 'Document';
	if (extension) extension = `${kind}_${extension.toLowerCase()}`;
	if (!isDark) {
		kind = kind + '_Light';
		document = 'Document_Light';
		if (extension) extension = extension + '_Light';
	}

	return icons[
		extension && isKeyOf(icons, extension) ? extension : isKeyOf(icons, kind) ? kind : document
	];
};

interface Props {
	data: ExplorerItem;
	size: null | number;
	className?: string;
	loadOriginal?: boolean;
	forceShowExtension?: boolean;
}

export default function Thumb(props: Props) {
	const isDark = useIsDark();
	const thumbRef = useRef<HTMLImageElement>(null);
	const platform = usePlatform();
	const { library } = useLibraryContext();
	const { locationId } = useExplorerStore();
	const [thumbSize, setThumbSize] = useState<null | { width: number; height: number }>(null);
	const { cas_id, isDir, kind, hasThumbnail, extension } = getExplorerItemData(props.data);

	// Only Videos and Images can show the original file
	if (props.loadOriginal && kind !== 'Video' && kind !== 'Image') props.loadOriginal = false;

	const src = hasThumbnail
		? props.loadOriginal && locationId
			? platform.getFileUrl(library.uuid, locationId, props.data.item.id)
			: cas_id && platform.getThumbnailUrlById(cas_id)
		: null;

	let style = {};
	if (props.size && kind === 'Video') {
		const videoBarsHeight = Math.floor(props.size / 10);
		style = {
			borderTopWidth: videoBarsHeight,
			borderBottomWidth: videoBarsHeight
		};
	}

	const childClassName = clsx(
		'z-90 pointer-events-none h-auto max-h-full w-auto max-w-full object-cover'
	);

	return (
		<div
			style={props.size ? { width: props.size, height: props.size } : {}}
			className={clsx(
				props.size && 'shrink-0',
				'relative flex items-center justify-center justify-items-center border-2 border-transparent p-1',
				props.className
			)}
		>
			{src ? (
				kind === 'Video' && props.loadOriginal ? (
					<video
						src={src}
						onCanPlay={(e) => {
							const video = e.target as HTMLVideoElement;
							// Why not use the element attribute? Because React...
							// https://github.com/facebook/react/issues/10389
							video.loop = true;
							video.muted = true;
						}}
						style={style}
						autoPlay
						className={clsx(
							childClassName,
							kind === 'Video' && 'rounded border-x-0 !border-black'
						)}
						playsInline
					/>
				) : (
					<>
						<img
							src={src}
							ref={thumbRef}
							style={style}
							onLoad={(e) => {
								const { width, height } = e.target as HTMLImageElement;
								setThumbSize(
									width === 0 || height === 0 ? null : { width, height }
								);
							}}
							className={clsx(
								childClassName,
								'rounded-sm shadow shadow-black/30',
								kind === 'Image' && classes.checkers,
								kind === 'Image' &&
									props.size &&
									props.size > 60 &&
									'border-2 border-app-line',
								kind === 'Video' && 'rounded border-x-0 !border-black'
							)}
						/>
						{kind === 'Video' &&
							thumbSize &&
							props.size &&
							(props.size > 80 || props.forceShowExtension) && (
								<div
									style={{
										marginTop: Math.floor(thumbSize.height / 2) - 2,
										marginLeft: Math.floor(thumbSize.width / 2) - 2
									}}
									className="absolute left-1/2 top-1/2 -translate-x-full -translate-y-full rounded bg-black/60 py-0.5 px-1 text-[9px] font-semibold uppercase opacity-70"
								>
									{extension}
								</div>
							)}
					</>
				)
			) : (
				<img
					src={getIcon(isDir, isDark, kind, extension)}
					decoding="async"
					className={childClassName}
				/>
			)}
		</div>
	);
}
