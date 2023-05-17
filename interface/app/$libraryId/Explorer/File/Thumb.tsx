import { getIcon } from '@sd/assets/icons/util';
import clsx from 'clsx';
import { ImgHTMLAttributes, memo, useEffect, useRef, useState } from 'react';
import { ExplorerItem, useLibraryContext } from '@sd/client';
import { useCallbackToWatchResize, useExplorerStore, useIsDark } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { pdfViewerEnabled } from '~/util/pdfViewer';
import { getExplorerItemData } from '../util';
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

const Thumbnail = ({
	src,
	cover,
	onLoad,
	onError,
	decoding,
	className,
	crossOrigin,
	videoBarsSize,
	videoExtension
}: ThumbnailProps) => {
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
				src={src}
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
				onLoad={onLoad}
				onError={() => {
					onError?.();
					setSize(null);
				}}
				decoding={decoding}
				className={className}
			/>
			{videoExtension && (
				<div
					style={
						cover
							? {}
							: size
							? {
									marginTop: Math.floor(size.height / 2) - 2,
									marginLeft: Math.floor(size.width / 2) - 2
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
					{videoExtension}
				</div>
			)}
		</>
	);
};

enum ThumbType {
	Icon,
	Original,
	Thumbnail
}

export interface ThumbProps {
	data: ExplorerItem;
	size: null | number;
	cover?: boolean;
	className?: string;
	loadOriginal?: boolean;
	mediaControls?: boolean;
}

function FileThumb({ size, cover, ...props }: ThumbProps) {
	const isDark = useIsDark();
	const platform = usePlatform();
	const { library } = useLibraryContext();
	const [src, setSrc] = useState<string>('#');
	const [thumbType, setThumbType] = useState(ThumbType.Icon);
	const { locationId, newThumbnails } = useExplorerStore();
	const { kind, extension, hasThumbnail, cas_id, isDir } = getExplorerItemData(
		props.data,
		newThumbnails
	);

	useEffect(() => {
		if (props.loadOriginal) {
			setThumbType(ThumbType.Original);
		} else if (hasThumbnail) {
			setThumbType(ThumbType.Thumbnail);
		} else {
			setThumbType(ThumbType.Icon);
		}
	}, [props.loadOriginal, props.data, hasThumbnail]);

	useEffect(() => {
		switch (thumbType) {
			case ThumbType.Original:
				if (locationId) {
					setSrc(platform.getFileUrl(library.uuid, locationId, props.data.item.id));
				} else {
					setThumbType(ThumbType.Thumbnail);
				}
				break;
			case ThumbType.Thumbnail:
				if (cas_id) {
					setSrc(platform.getThumbnailUrlById(cas_id));
				} else {
					setThumbType(ThumbType.Icon);
				}
				break;
			default:
				setSrc(getIcon(kind, isDir, isDark, extension));
				break;
		}
	}, [
		kind,
		props.data.item.id,
		isDir,
		isDark,
		cas_id,
		library.uuid,
		platform,
		thumbType,
		extension,
		locationId
	]);

	const childClassName = 'max-h-full max-w-full object-contain';
	return (
		<div
			style={size ? { maxWidth: size, width: size - 10, height: size } : {}}
			className={clsx(
				'relative flex shrink-0 items-center justify-center',
				size && !(ThumbType.Original && kind === 'Video') && 'border-2 border-transparent',
				size || ['h-full', cover ? 'w-full overflow-hidden' : 'w-[90%]'],
				props.className
			)}
		>
			{(() => {
				switch (thumbType) {
					case ThumbType.Original: {
						const onError = () =>
							void setThumbType(hasThumbnail ? ThumbType.Thumbnail : ThumbType.Icon);

						switch (extension === 'pdf' && pdfViewerEnabled() ? 'PDF' : kind) {
							case 'PDF':
								return (
									<object
										data={src}
										type="application/pdf"
										className={clsx(
											'h-full w-full border-0',
											childClassName,
											props.className
										)}
									/>
								);
							case 'Video':
								return (
									<video
										crossOrigin="anonymous"
										src={src}
										onError={onError}
										autoPlay
										controls={props.mediaControls}
										onCanPlay={(e) => {
											const video = e.target as HTMLVideoElement;
											// Why not use the element's attribute? Because React...
											// https://github.com/facebook/react/issues/10389
											video.loop = !props.mediaControls;
											video.muted = !props.mediaControls;
										}}
										className={clsx(
											childClassName,
											size && 'rounded border-x-0 border-black',
											props.className
										)}
										playsInline
									>
										<p>Video preview is not supported.</p>
									</video>
								);
							case 'Audio':
								return (
									<>
										<img
											src={getIcon('Audio', false, isDark, extension)}
											decoding={size ? 'async' : 'sync'}
											className={clsx(childClassName, props.className)}
										/>
										{props.mediaControls && (
											<audio
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
					case ThumbType.Thumbnail:
						return (
							<Thumbnail
								src={src}
								cover={cover}
								onError={() => {
									setThumbType(ThumbType.Icon);
								}}
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
								crossOrigin={ThumbType.Original && 'anonymous'}
								videoBarsSize={
									(kind === 'Video' && size && Math.floor(size / 10)) || 0
								}
								videoExtension={
									(kind === 'Video' &&
										(size == null || size > 80) &&
										extension) ||
									''
								}
							/>
						);
					default:
						return (
							<img
								src={src}
								decoding={size ? 'async' : 'sync'}
								className={clsx(childClassName, props.className)}
							/>
						);
				}
			})()}
		</div>
	);
}

export default memo(FileThumb);
