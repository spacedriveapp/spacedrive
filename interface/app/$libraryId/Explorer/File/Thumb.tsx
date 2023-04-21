import * as icons from '@sd/assets/icons';
import clsx from 'clsx';
import { useLayoutEffect, useRef, useState } from 'react';
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
	cover?: boolean;
	className?: string;
	loadOriginal?: boolean;
}

export default function Thumb({ size, ...props }: Props) {
	const isDark = useIsDark();
	const platform = usePlatform();
	const { library } = useLibraryContext();
	const { locationId } = useExplorerStore();
	const [image, setImage] = useState<HTMLImageElement>();
	const [thumbSize, setThumbSize] = useState<null | { width: number; height: number }>(null);
	const { cas_id, isDir, kind, hasThumbnail, extension } = getExplorerItemData(props.data);

	useLayoutEffect(() => {
		if (!image) return;

		// This is needed because the image might not be loaded yet
		// https://stackoverflow.com/q/61864491#61864635
		let counter = 0;
		const waitImageRender = () => {
			const { width, height } = image;
			if (width && height) {
				setThumbSize({ width, height });
			} else if (++counter < 3) {
				requestAnimationFrame(waitImageRender);
			} else {
				setThumbSize(null);
			}
		};

		waitImageRender();

		return () => {
			counter = 3;
		};
	}, [image]);

	// Only Videos and Images can show the original file
	const loadOriginal = (kind === 'Video' || kind === 'Image') && props.loadOriginal;
	const src = hasThumbnail
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
				src &&
					kind !== 'Video' && [classes.checkers, size && 'border-2 border-transparent'],
				props.className,
				size || ['h-full', props.cover ? 'w-full overflow-hidden' : 'w-[90%]']
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
						className={clsx(childClassName, size && 'rounded border-x-0 border-black')}
						playsInline
					/>
				) : (
					<>
						<img
							src={src}
							style={style}
							onLoad={(e) => {
								if (kind === 'Video' && !props.cover)
									setImage(e.target as HTMLImageElement);
							}}
							decoding="async"
							className={clsx(
								props.cover
									? 'min-h-full min-w-full object-cover object-center'
									: childClassName,
								'shadow shadow-black/30',
								kind === 'Video' ? 'rounded' : 'rounded-sm',
								size &&
									(kind === 'Video'
										? 'border-x-0 border-black'
										: size > 60 && 'border-2 border-app-line')
							)}
						/>
						{kind === 'Video' && (!size || size > 80) && (
							<div
								style={
									props.cover || thumbSize == null
										? {}
										: {
												marginTop: Math.floor(thumbSize.height / 2) - 2,
												marginLeft: Math.floor(thumbSize.width / 2) - 2
										  }
								}
								className={clsx(
									props.cover
										? 'right-1 bottom-1'
										: 'left-1/2 top-1/2 -translate-x-full -translate-y-full',
									'absolute rounded',
									'bg-black/60 py-0.5 px-1 text-[9px] font-semibold uppercase opacity-70'
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
					className={childClassName}
				/>
			)}
		</div>
	);
}
