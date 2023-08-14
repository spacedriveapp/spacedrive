import { getIcon, iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import {
	ImgHTMLAttributes,
	VideoHTMLAttributes,
	memo,
	useEffect,
	useLayoutEffect,
	useRef,
	useState
} from 'react';
import { ExplorerItem, getItemFilePath, getItemLocation, useLibraryContext } from '@sd/client';
import { PDFViewer, TEXTViewer } from '~/components';
import { useCallbackToWatchResize, useIsDark } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { pdfViewerEnabled } from '~/util/pdfViewer';
import { useExplorerContext } from '../Context';
import { getExplorerStore } from '../store';
import { useExplorerItemData } from '../util';
import classes from './Thumb.module.scss';

const THUMB_TYPE = {
	ICON: 'icon',
	ORIGINAL: 'original',
	THUMBNAIL: 'thumbnail'
} as const;

type ThumbType = (typeof THUMB_TYPE)[keyof typeof THUMB_TYPE];

export interface ThumbProps {
	data: ExplorerItem;
	loadOriginal?: boolean;
	size?: number;
	cover?: boolean;
	frame?: boolean;
	mediaControls?: boolean;
	pauseVideo?: boolean;
	className?: string;
	childClassName?: string | ((type: ThumbType) => string | undefined);
}

export const FileThumb = memo((props: ThumbProps) => {
	const isDark = useIsDark();
	const platform = usePlatform();

	const itemData = useExplorerItemData(props.data);
	const filePath = getItemFilePath(props.data);

	const { parent } = useExplorerContext();
	const { library } = useLibraryContext();

	const [src, setSrc] = useState<string>();
	const [loaded, setLoaded] = useState<boolean>(false);
	const [thumbType, setThumbType] = useState<ThumbType>('icon');

	const childClassName = 'max-h-full max-w-full object-contain';
	const frameClassName = clsx(
		'rounded-sm border-2 border-app-line bg-app-darkBox',
		isDark ? classes.checkers : classes.checkersLight
	);

	const onLoad = () => setLoaded(true);

	const onError = () => {
		setLoaded(false);
		setThumbType((prevThumbType) =>
			prevThumbType === 'original' && itemData.hasLocalThumbnail ? 'thumbnail' : 'icon'
		);
	};

	// useLayoutEffect is required to ensure the thumbType is always updated before the onError listener can execute,
	// thus avoiding improper thumb types changes
	useLayoutEffect(() => {
		// Reset src when item changes, to allow detection of yet not updated src
		setSrc(undefined);
		setLoaded(false);

		if (props.loadOriginal) setThumbType('original');
		else if (itemData.hasLocalThumbnail) setThumbType('thumbnail');
		else setThumbType('icon');
	}, [props.loadOriginal, itemData]);

	useEffect(() => {
		const locationId =
			itemData.locationId ?? (parent?.type === 'Location' ? parent.location.id : null);

		switch (thumbType) {
			case 'original':
				if (locationId === null) setThumbType('thumbnail');
				else {
					setSrc(
						platform.getFileUrl(
							library.uuid,
							locationId,
							filePath?.id || props.data.item.id,
							// Workaround Linux webview not supporting playing video and audio through custom protocol urls
							itemData.kind == 'Video' || itemData.kind == 'Audio'
						)
					);
				}
				break;

			case 'thumbnail':
				if (!itemData.casId || !itemData.thumbnailKey) setThumbType('icon');
				else setSrc(platform.getThumbnailUrlByThumbKey(itemData.thumbnailKey));
				break;

			default:
				setSrc(
					getIcon(
						itemData.isDir ? 'Folder' : itemData.kind,
						isDark,
						itemData.extension,
						itemData.isDir
					)
				);
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

	return (
		<div
			style={{
				...(props.size
					? { maxWidth: props.size, width: props.size, height: props.size }
					: {})
			}}
			className={clsx(
				'relative flex shrink-0 items-center justify-center',
				loaded ? 'visible' : 'invisible',
				!props.size && 'h-full w-full',
				props.cover && 'overflow-hidden',
				props.className
			)}
		>
			{(() => {
				if (!src) return;

				const className = clsx(
					childClassName,
					typeof props.childClassName === 'function'
						? props.childClassName(thumbType)
						: props.childClassName
				);

				switch (thumbType) {
					case 'original': {
						switch (itemData.extension === 'pdf' ? 'PDF' : itemData.kind) {
							case 'PDF':
								if (!pdfViewerEnabled()) return;
								return (
									<PDFViewer
										src={src}
										onLoad={onLoad}
										onError={onError}
										className={clsx(
											'h-full w-full',
											className,
											props.frame && frameClassName
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
											'h-full w-full px-4 font-mono',
											!props.mediaControls
												? 'overflow-hidden'
												: 'overflow-auto',
											className,
											props.frame && [frameClassName, '!bg-none']
										)}
										crossOrigin="anonymous"
									/>
								);

							case 'Video':
								return (
									<Video
										src={src}
										onLoadedData={onLoad}
										onError={onError}
										paused={props.pauseVideo}
										controls={props.mediaControls}
										className={clsx(className, props.frame && frameClassName)}
									/>
								);

							case 'Audio':
								return (
									<>
										<img
											src={getIcon(
												iconNames.Audio,
												isDark,
												itemData.extension
											)}
											onLoad={onLoad}
											decoding={props.size ? 'async' : 'sync'}
											className={childClassName}
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
					}

					// eslint-disable-next-line no-fallthrough
					case 'thumbnail':
						return (
							<Thumbnail
								src={src}
								cover={props.cover}
								onLoad={onLoad}
								onError={onError}
								decoding={props.size ? 'async' : 'sync'}
								className={clsx(
									props.cover
										? 'min-h-full min-w-full object-cover object-center'
										: className,

									props.frame &&
										(itemData.kind !== 'Video' || thumbType == 'original')
										? frameClassName
										: null
								)}
								crossOrigin={thumbType !== 'original' ? 'anonymous' : undefined} // Here it is ok, because it is not a react attr
								videoBars={itemData.kind === 'Video' && !props.cover}
								extension={
									itemData.extension && itemData.kind === 'Video'
										? itemData.extension
										: undefined
								}
							/>
						);

					default:
						return (
							<img
								src={src}
								onLoad={onLoad}
								onError={() => setLoaded(false)}
								decoding={props.size ? 'async' : 'sync'}
								className={childClassName}
								draggable={false}
							/>
						);
				}
			})()}
		</div>
	);
});

interface ThumbnailProps extends ImgHTMLAttributes<HTMLImageElement> {
	cover?: boolean;
	videoBars?: boolean;
	extension?: string;
}

const Thumbnail = memo(
	({
		crossOrigin,
		videoBars,
		extension,
		cover,
		onError,
		className,
		...props
	}: ThumbnailProps) => {
		const ref = useRef<HTMLImageElement>(null);

		const [size, setSize] = useState<{ width: number; height: number }>();

		useCallbackToWatchResize(({ width, height }) => setSize({ width, height }), [], ref);

		const videoBarSize = (size: number) => Math.floor(size / 10);

		return (
			<>
				<img
					// Order matter for crossOrigin attr
					// https://github.com/facebook/react/issues/14035#issuecomment-642227899
					{...(crossOrigin ? { crossOrigin } : {})}
					ref={ref}
					onError={(e) => {
						onError?.(e);
						setSize(undefined);
					}}
					draggable={false}
					className={clsx(className, videoBars && 'rounded border-black')}
					style={
						videoBars
							? size
								? size.height >= size.width
									? {
											borderLeftWidth: videoBarSize(size.height),
											borderRightWidth: videoBarSize(size.height)
									  }
									: {
											borderTopWidth: videoBarSize(size.width),
											borderBottomWidth: videoBarSize(size.width)
									  }
								: {}
							: {}
					}
					{...props}
				/>

				{(cover || (size && size.width > 80)) && extension && (
					<div
						style={{
							...(!cover &&
								size && {
									marginTop: Math.floor(size.height / 2) - 2,
									marginLeft: Math.floor(size.width / 2) - 2
								})
						}}
						className={clsx(
							'absolute rounded bg-black/60 px-1 py-0.5 text-[9px] font-semibold uppercase text-white opacity-70',
							cover
								? 'bottom-1 right-1'
								: 'left-1/2 top-1/2 -translate-x-full -translate-y-full'
						)}
					>
						{extension}
					</div>
				)}
			</>
		);
	}
);

interface VideoProps extends VideoHTMLAttributes<HTMLVideoElement> {
	paused?: boolean;
}

const Video = memo(({ paused, ...props }: VideoProps) => {
	const ref = useRef<HTMLVideoElement>(null);

	useEffect(() => {
		if (!ref.current) return;
		paused ? ref.current.pause() : ref.current.play();
	}, [paused]);

	return (
		<video
			// Order matter for crossOrigin attr
			crossOrigin="anonymous"
			ref={ref}
			autoPlay={!paused}
			onVolumeChange={(e) => {
				const video = e.target as HTMLVideoElement;
				getExplorerStore().mediaPlayerVolume = video.volume;
			}}
			onCanPlay={(e) => {
				const video = e.target as HTMLVideoElement;
				// Why not use the element's attribute? Because React...
				// https://github.com/facebook/react/issues/10389
				video.loop = !props.controls;
				video.muted = !props.controls;
				video.volume = getExplorerStore().mediaPlayerVolume;
			}}
			playsInline
			draggable={false}
			{...props}
		>
			<p>Video preview is not supported.</p>
		</video>
	);
});
