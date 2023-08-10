import { getIcon, iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import { ImgHTMLAttributes, memo, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ExplorerItem, getItemFilePath, getItemLocation, useLibraryContext } from '@sd/client';
import { PDFViewer, TEXTViewer } from '~/components';
import { useCallbackToWatchResize, useIsDark } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { pdfViewerEnabled } from '~/util/pdfViewer';
import { useExplorerContext } from '../Context';
import { getExplorerStore } from '../store';
import { useExplorerItemData } from '../util';
import classes from './Thumb.module.scss';

interface ThumbnailProps {
	src: string;
	cover?: boolean;
	onLoad?: () => void;
	onError?: () => void;
	decoding?: ImgHTMLAttributes<HTMLImageElement>['decoding'];
	className?: string;
	crossOrigin?: ImgHTMLAttributes<HTMLImageElement>['crossOrigin'];
	videoBarsSize?: number;
	videoExtension?: string;
}

const Thumbnail = memo(
	({ crossOrigin, videoBarsSize, videoExtension, ...props }: ThumbnailProps) => {
		const ref = useRef<HTMLImageElement>(null);
		const [size, setSize] = useState<null | { width: number; height: number }>(null);

		useCallbackToWatchResize(
			(rect) => {
				const { width, height } = rect;
				setSize((width && height && { width, height }) || null);
			},
			[],
			ref
		);

		return (
			<>
				<img
					// Order matter for crossOrigin attr
					// https://github.com/facebook/react/issues/14035#issuecomment-642227899
					{...(crossOrigin ? { crossOrigin } : {})}
					src={props.src}
					ref={ref}
					style={
						videoBarsSize
							? size && size.height >= size.width
								? {
										borderLeftWidth: videoBarsSize,
										borderRightWidth: videoBarsSize
								  }
								: {
										borderTopWidth: videoBarsSize,
										borderBottomWidth: videoBarsSize
								  }
							: {}
					}
					onLoad={props.onLoad}
					onError={() => {
						props.onError?.();
						setSize(null);
					}}
					decoding={props.decoding}
					className={props.className}
					draggable={false}
				/>
				{videoExtension && (
					<div
						style={
							props.cover
								? {}
								: size
								? {
										marginTop: Math.floor(size.height / 2) - 2,
										marginLeft: Math.floor(size.width / 2) - 2
								  }
								: { display: 'none' }
						}
						className={clsx(
							props.cover
								? 'bottom-1 right-1'
								: 'left-1/2 top-1/2 -translate-x-full -translate-y-full',
							'absolute rounded !text-white',
							'bg-black/60 px-1 py-0.5 text-[9px] font-semibold uppercase opacity-70'
						)}
					>
						{videoExtension}
					</div>
				)}
			</>
		);
	}
);

enum ThumbType {
	Icon,
	Original,
	Thumbnail,
	Location
}

export interface ThumbProps {
	data: ExplorerItem;
	size: null | number;
	cover?: boolean;
	className?: string;
	loadOriginal?: boolean;
	mediaControls?: boolean;
	pauseVideo?: boolean;
}

function FileThumb({ size, cover, ...props }: ThumbProps) {
	const isDark = useIsDark();
	const platform = usePlatform();
	const itemData = useExplorerItemData(props.data);
	const locationData = getItemLocation(props.data);
	const filePath = getItemFilePath(props.data);
	const { library } = useLibraryContext();
	const [src, setSrc] = useState<null | string>(null);
	const [loaded, setLoaded] = useState<boolean>(false);
	const [thumbType, setThumbType] = useState(ThumbType.Icon);
	const { parent } = useExplorerContext();

	// useLayoutEffect is required to ensure the thumbType is always updated before the onError listener can execute,
	// thus avoiding improper thumb types changes
	useLayoutEffect(() => {
		// Reset src when item changes, to allow detection of yet not updated src
		setSrc(null);
		setLoaded(false);

		if (locationData) {
			setThumbType(ThumbType.Location);
		} else if (props.loadOriginal) {
			setThumbType(ThumbType.Original);
		} else if (itemData.hasLocalThumbnail) {
			setThumbType(ThumbType.Thumbnail);
		} else {
			setThumbType(ThumbType.Icon);
		}
	}, [props.loadOriginal, locationData, itemData]);

	useEffect(() => {
		const {
			casId,
			kind,
			isDir,
			extension,
			locationId: itemLocationId,
			thumbnailKey
		} = itemData;
		const locationId =
			itemLocationId ?? (parent?.type === 'Location' ? parent.location.id : null);

		switch (thumbType) {
			case ThumbType.Original:
				if (locationId) {
					setSrc(
						platform.getFileUrl(
							library.uuid,
							locationId,
							filePath?.id || props.data.item.id,
							// Workaround Linux webview not supporting playing video and audio through custom protocol urls
							kind == 'Video' || kind == 'Audio'
						)
					);
				} else {
					setThumbType(ThumbType.Thumbnail);
				}
				break;
			case ThumbType.Thumbnail:
				if (casId && thumbnailKey) {
					setSrc(platform.getThumbnailUrlByThumbKey(thumbnailKey));
				} else {
					setThumbType(ThumbType.Icon);
				}
				break;
			case ThumbType.Location:
				setSrc(getIcon('Folder', isDark, extension, true));
				break;
			default:
				if (isDir !== null) setSrc(getIcon(kind, isDark, extension, isDir));
				break;
		}
	}, [
		props.data.item.id,
		filePath?.id,
		isDark,
		library.uuid,
		itemData,
		platform,
		thumbType,
		parent
	]);

	const onLoad = () => setLoaded(true);

	const onError = () => {
		setLoaded(false);
		setThumbType((prevThumbType) => {
			return prevThumbType === ThumbType.Original && itemData.hasLocalThumbnail
				? ThumbType.Thumbnail
				: ThumbType.Icon;
		});
	};

	const { kind, extension } = itemData;
	const childClassName = 'max-h-full max-w-full object-contain';
	return (
		<div
			style={{
				visibility: loaded ? 'visible' : 'hidden',
				...(size ? { maxWidth: size, width: size - 10, height: size } : {})
			}}
			className={clsx(
				'relative flex shrink-0 items-center justify-center',
				size &&
					kind !== 'Video' &&
					thumbType !== ThumbType.Icon &&
					'border-2 border-transparent',
				size || ['h-full', cover ? 'w-full overflow-hidden' : 'w-[90%]'],
				props.className
			)}
		>
			{(() => {
				if (src == null) return null;
				switch (thumbType) {
					case ThumbType.Original:
						switch (extension === 'pdf' && pdfViewerEnabled() ? 'PDF' : kind) {
							case 'PDF':
								return (
									<PDFViewer
										src={src}
										onLoad={onLoad}
										onError={onError}
										className={clsx(
											'h-full w-full border-0',
											childClassName,
											props.className
										)}
										crossOrigin="anonymous" // Here it is ok, because it is not a react attr
									/>
								);
							case 'Text':
								return (
									<TEXTViewer
										src={src}
										onLoad={onLoad}
										onError={onError}
										className={clsx(
											'h-full w-full border-0 font-mono px-4',
											!props.mediaControls ? 'overflow-hidden' : 'overflow-auto',
											childClassName,
											props.className
										)}
										crossOrigin="anonymous"
									/>
								);
							case 'Video':
								return (
									<Video
										src={src}
										onLoad={onLoad}
										onError={onError}
										paused={props.pauseVideo}
										controls={props.mediaControls}
										className={clsx(
											childClassName,
											size && 'rounded border-x-0 border-black',
											props.className
										)}
									/>
								);
							case 'Audio':
								return (
									<>
										<img
											src={getIcon(iconNames.Audio, isDark, extension)}
											onLoad={onLoad}
											decoding={size ? 'async' : 'sync'}
											className={clsx(childClassName, props.className)}
											draggable={false}
										/>
										{props.mediaControls && (
											<audio
												// Order matter for crossOrigin attr
												crossOrigin="anonymous"
												src={src}
												onError={onError}
												controls
												autoPlay
												className="absolute left-2/4 top-full w-full -translate-x-1/2 translate-y-[-150%]"
											>
												<p>Audio preview is not supported.</p>
											</audio>
										)}
									</>
								);
						}
					// eslint-disable-next-line no-fallthrough
					case ThumbType.Thumbnail:
						return (
							<Thumbnail
								src={src}
								cover={cover}
								onLoad={onLoad}
								onError={onError}
								decoding={size ? 'async' : 'sync'}
								className={clsx(
									cover
										? 'min-h-full min-w-full object-cover object-center'
										: childClassName,
									kind === 'Video' ? 'rounded' : 'rounded-sm',
									ThumbType.Original || [
										classes.checkers,
										'shadow shadow-black/30'
									],
									size &&
										(kind === 'Video'
											? 'border-x-0 border-black'
											: size > 60 && 'border-2 border-app-line'),
									props.className
								)}
								crossOrigin={ThumbType.Original && 'anonymous'} // Here it is ok, because it is not a react attr
								videoBarsSize={
									(kind === 'Video' && size && Math.floor(size / 10)) || 0
								}
								videoExtension={
									(kind === 'Video' &&
										(cover || size == null || size > 80) &&
										extension) ||
									''
								}
							/>
						);
					default:
						return (
							<img
								src={src}
								onLoad={onLoad}
								onError={() => setLoaded(false)}
								decoding={size ? 'async' : 'sync'}
								className={clsx(childClassName, props.className)}
								draggable={false}
							/>
						);
				}
			})()}
		</div>
	);
}

export default memo(FileThumb);

interface VideoProps {
	src: string;
	paused?: boolean;
	controls?: boolean;
	className?: string;
	onLoad?: () => void;
	onError?: () => void;
}

const Video = memo(({ src, paused, controls, className, onLoad, onError }: VideoProps) => {
	const video = useRef<HTMLVideoElement>(null);

	useEffect(() => {
		if (video.current) {
			paused ? video.current.pause() : video.current.play();
		}
	}, [paused]);

	return (
		<video
			// Order matter for crossOrigin attr
			crossOrigin="anonymous"
			ref={video}
			src={src}
			onError={onError}
			autoPlay={!paused}
			onVolumeChange={(e) => {
				const video = e.target as HTMLVideoElement;
				getExplorerStore().mediaPlayerVolume = video.volume;
			}}
			controls={controls}
			onCanPlay={(e) => {
				const video = e.target as HTMLVideoElement;
				// Why not use the element's attribute? Because React...
				// https://github.com/facebook/react/issues/10389
				video.loop = !controls;
				video.muted = !controls;
				video.volume = getExplorerStore().mediaPlayerVolume;
			}}
			className={className}
			playsInline
			onLoadedData={onLoad}
			draggable={false}
		>
			<p>Video preview is not supported.</p>
		</video>
	);
});
