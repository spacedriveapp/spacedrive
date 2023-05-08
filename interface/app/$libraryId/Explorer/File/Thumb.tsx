import * as icons from '@sd/assets/icons';
import clsx from 'clsx';
import { memo, useEffect, useLayoutEffect, useRef, useState } from 'react';
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

interface VideoThumbSize {
	width: number;
	height: number;
}

export interface ThumbProps {
	data: ExplorerItem;
	size: null | number;
	cover?: boolean;
	className?: string;
	loadOriginal?: boolean;
}

function Thumb({ size, cover, ...props }: ThumbProps) {
	const isDark = useIsDark();
	const platform = usePlatform();
	const thumbImg = useRef<HTMLImageElement>(null);
	const [thumbSize, setThumbSize] = useState<null | VideoThumbSize>(null);
	const { library } = useLibraryContext();
	const [thumbLoaded, setThumbLoaded] = useState<boolean>(false);
	const { locationId, newThumbnails } = useExplorerStore();

	const { cas_id, isDir, kind, hasThumbnail, newThumb, extension } = getExplorerItemData(
		props.data,
		newThumbnails
	);

	// Allows disabling thumbnails when they fail to load
	const [useThumb, setUseThumb] = useState<boolean>(hasThumbnail);

	// When new thumbnails are generated, reset the useThumb state
	// If it fails to load, it will be set back to false by the error handler in the img
	useEffect(() => {
		if (newThumb) setUseThumb(true);
	}, [newThumb]);

	useLayoutEffect(() => {
		const img = thumbImg.current;
		if (cover || kind !== 'Video' || !img || !thumbLoaded) return;

		const resizeObserver = new ResizeObserver(() => {
			const { width, height } = img;
			setThumbSize(width && height ? { width, height } : null);
		});

		resizeObserver.observe(img);
		return () => resizeObserver.disconnect();
	}, [kind, cover, thumbImg, thumbLoaded]);

	// Only Videos and Images can show the original file
	const loadOriginal = (kind === 'Video' || kind === 'Image') && props.loadOriginal;
	const src = useThumb
		? loadOriginal && locationId
			? platform.getFileUrl(library.uuid, locationId, props.data.item.id)
			: cas_id && platform.getThumbnailUrlById(cas_id)
		: null;

	let style = {};
	if (size && kind === 'Video') {
		const videoBarsHeight = Math.floor(size / 10);
		style = {
			borderTopWidth: videoBarsHeight,
			borderBottomWidth: videoBarsHeight
		};
	}

	const childClassName = 'max-h-full max-w-full object-contain';
	return (
		<div
			style={size ? { maxWidth: size, width: size - 10, height: size } : {}}
			className={clsx(
				'relative flex shrink-0 items-center justify-center',
				src && kind !== 'Video' && [size && 'border-2 border-transparent'],
				size || ['h-full', cover ? 'w-full overflow-hidden' : 'w-[90%]'],
				props.className
			)}
		>
			{src ? (
				kind === 'Video' && loadOriginal ? (
					<video
						src={src}
						onCanPlay={(e) => {
							const video = e.target as HTMLVideoElement;
							// Why not use the element's attribute? Because React...
							// https://github.com/facebook/react/issues/10389
							video.loop = true;
							video.muted = true;
						}}
						style={style}
						autoPlay
						className={clsx(
							childClassName,
							size && 'rounded border-x-0 border-black',
							props.className
						)}
						playsInline
					/>
				) : (
					<>
						<img
							src={src}
							ref={thumbImg}
							style={style}
							onLoad={() => {
								setUseThumb(true);
								setThumbLoaded(true);
							}}
							onError={() => {
								setUseThumb(false);
								setThumbSize(null);
								setThumbLoaded(false);
							}}
							decoding="async"
							className={clsx(
								cover
									? 'min-h-full min-w-full object-cover object-center'
									: childClassName,
								'shadow shadow-black/30',
								kind === 'Video' ? 'rounded' : 'rounded-sm',
								classes.checkers,
								size &&
								(kind === 'Video'
									? 'border-x-0 border-black'
									: size > 60 && 'border-2 border-app-line'),
								props.className
							)}
						/>
						{kind === 'Video' && (!size || size > 80) && (
							<div
								style={
									cover
										? {}
										: thumbSize
											? {
												marginTop: Math.floor(thumbSize.height / 2) - 2,
												marginLeft: Math.floor(thumbSize.width / 2) - 2
											}
											: { display: 'none' }
								}
								className={clsx(
									cover
										? 'bottom-1 right-1'
										: 'left-1/2 top-1/2 -translate-x-full -translate-y-full',
									'absolute rounded',
									'bg-black/60 px-1 py-0.5 text-[9px] font-semibold uppercase opacity-70'
								)}
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
					className={clsx(childClassName, props.className)}
				/>
			)}
		</div>
	);
}

export default Thumb;
