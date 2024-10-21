import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import {
	ComponentProps,
	ErrorInfo,
	forwardRef,
	HTMLAttributes,
	memo,
	SyntheticEvent,
	useCallback,
	useEffect,
	useImperativeHandle,
	useMemo,
	useRef,
	useState
} from 'react';
import { getItemFilePath, ObjectKindKey, useLibraryContext, type ExplorerItem } from '@sd/client';
import { pdfViewerEnabled } from '~/util/pdfViewer';
import { usePlatform } from '~/util/Platform';

import { explorerStore } from '../store';
import { useExplorerItemData } from '../useExplorerItemData';
import { ErrorBarrier } from './ErrorBarrier';
import { Image } from './Image';
import LayeredFileIcon from './LayeredFileIcon';
import { Original } from './Original';
import { useFrame } from './useFrame';
import { useBlackBars, useSize } from './utils';

export type ThumbType = 'original' | 'thumbnail' | 'icon';

type LoadState = {
	[K in ThumbType]: 'normal' | 'error';
};

const ThumbClasses = 'max-h-full max-w-full object-contain';

interface ThumbnailProps extends ComponentProps<'img'> {
	cover?: boolean;
	blackBars?: boolean;
	blackBarsSize?: number;
	videoExtension?: string;
}

const Thumbnail = memo(
	forwardRef<HTMLImageElement, ThumbnailProps>(
		(
			{
				blackBars,
				blackBarsSize,
				videoExtension: extension,
				cover,
				className,
				style,
				...props
			},
			_ref
		) => {
			const ref = useRef<HTMLImageElement>(null);
			useImperativeHandle<HTMLImageElement | null, HTMLImageElement | null>(
				_ref,
				() => ref.current
			);

			const size = useSize(ref);

			const { style: blackBarsStyle } = useBlackBars(ref, size, {
				size: blackBarsSize,
				disabled: !blackBars
			});

			return (
				<>
					<Image
						{...props}
						className={clsx(className, blackBars && size.width === 0 && 'invisible')}
						style={{ ...style, ...blackBarsStyle }}
						ref={ref}
					/>

					{(cover || size.width > 80) && extension && (
						<div
							style={{
								...(!cover && {
									marginTop: Math.floor(size.height / 2) - 2,
									marginLeft: Math.floor(size.width / 2) - 2
								})
							}}
							className={clsx(
								'pointer-events-none absolute rounded bg-black/60 px-1 py-0.5 text-[9px] font-semibold uppercase text-white opacity-70',
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
	)
);

interface ThumbProps extends ThumbnailProps {
	src?: string;
	kind: ObjectKindKey;
	path: string | null;
	isDir: boolean;
	frame: boolean;
	fileId: number | null;
	onLoad: () => void;
	onError: (error: Error | ErrorEvent | SyntheticEvent<Element, Event>) => void;
	thumbType: ThumbType;
	extension: string | null;
	customIcon: IconTypes | null;
	locationId: number | null;
	pauseVideo: boolean;
	magnification: number;
	mediaControls: boolean;
	frameClassName: string;
	isSidebarPreview: boolean;
}

const Thumb = memo(
	forwardRef<HTMLImageElement, ThumbProps>(
		(
			{
				src,
				kind,
				path,
				frame,
				isDir,
				cover,
				fileId,
				thumbType,
				extension,
				blackBars,
				className,
				pauseVideo,
				locationId,
				customIcon,
				magnification,
				mediaControls,
				blackBarsSize,
				videoExtension,
				frameClassName,
				isSidebarPreview,
				onLoad,
				...props
			},
			_ref
		) => {
			const ref = useRef<HTMLImageElement>(null);
			useImperativeHandle<HTMLImageElement | null, HTMLImageElement | null>(
				_ref,
				() => ref.current
			);
			const [isLoading, setIsLoading] = useState(true);

			const handleLoad = useCallback(() => {
				const img = ref.current;
				setIsLoading(!(img == null || (img.naturalHeight > 0 && img.naturalWidth > 0)));
				onLoad?.();
			}, [onLoad]);

			let thumb: JSX.Element | null = null;

			switch (thumbType) {
				case 'original':
					thumb = (
						<Original
							path={path}
							kind={kind}
							frame={frame}
							fileId={fileId}
							onLoad={handleLoad}
							extension={extension}
							blackBars={blackBars}
							className={clsx(ThumbClasses, className, isLoading && 'hidden')}
							locationId={locationId}
							pauseVideo={pauseVideo}
							blackBarsSize={blackBarsSize}
							magnification={magnification}
							mediaControls={mediaControls}
							frameClassName={frameClassName}
							childClassName={className}
							isSidebarPreview={isSidebarPreview}
						/>
					);
					break;
				case 'thumbnail':
					thumb = (
						<Thumbnail
							{...props}
							ref={ref}
							src={src}
							cover={cover}
							onLoad={handleLoad}
							decoding="async"
							className={clsx(
								cover
									? [
											'min-h-full min-w-full object-cover object-center',
											className
										]
									: [ThumbClasses, className],
								frame && !(kind === 'Video' && blackBars) ? frameClassName : null,
								isLoading && 'hidden'
							)}
							blackBars={blackBars && kind === 'Video' && !cover}
							crossOrigin="anonymous" // Here it is ok, because it is not a react attr
							blackBarsSize={blackBarsSize}
							videoExtension={videoExtension}
						/>
					);
					break;
			}

			return (
				<>
					<LayeredFileIcon
						{...props}
						ref={thumb == null ? ref : null}
						kind={kind}
						isDir={isDir}
						onLoad={thumb == null ? onLoad : () => {}}
						decoding="sync"
						draggable={false}
						extension={extension}
						className={clsx(ThumbClasses, className, !isLoading && 'hidden')}
						customIcon={customIcon}
					/>
					{thumb ?? null}
				</>
			);
		}
	)
);

export interface FileThumbProps {
	data: ExplorerItem;
	loadOriginal?: boolean;
	size?: number;
	cover?: boolean;
	frame?: boolean;
	onLoad?: (type: ThumbType) => void;
	onError?: (state: LoadState, error: Error) => void;
	blackBars?: boolean;
	blackBarsSize?: number;
	extension?: boolean;
	mediaControls?: boolean;
	pauseVideo?: boolean;
	className?: string;
	frameClassName?: string;
	childClassName?: string | ((type: ThumbType) => string | undefined);
	isSidebarPreview?: boolean;
	childProps?: HTMLAttributes<HTMLElement>;
	magnification?: number;
}

/**
 * This component is used to render a thumbnail of a file or folder.
 * It will automatically choose the best thumbnail to display based on the item data.
 *
 * .. WARNING::
 *    This Component is heavely used inside the explorer, and as such it is a performance critical component.
 * 	  Be careful with the performance of the code, make sure to always memoize any objects or functions to avoid unnecessary re-renders.
 *
 */
export const FileThumb = memo(
	forwardRef<HTMLImageElement, FileThumbProps>((props, ref) => {
		const frame = useFrame();
		const platform = usePlatform();
		const itemData = useExplorerItemData(props.data);
		const filePath = getItemFilePath(props.data);
		const { library } = useLibraryContext();
		const [loadState, setLoadState] = useState<LoadState>({
			icon: 'normal',
			original: 'normal',
			thumbnail: 'normal'
		});

		// WARNING: This is required so QuickPreview can work properly
		useEffect(() => {
			setLoadState({
				icon: 'normal',
				original: 'normal',
				thumbnail: 'normal'
			});
		}, [props.data]);

		const thumbType = useMemo((): ThumbType => {
			if (loadState.original !== 'error' && props.loadOriginal) return 'original';
			if (loadState.thumbnail !== 'error' && itemData.hasLocalThumbnail) return 'thumbnail';
			return 'icon';
		}, [itemData.hasLocalThumbnail, loadState, props.loadOriginal]);

		useEffect(() => {
			let timeoutId = null;
			// Reload thumbnail when it gets a notification from core that it has been generated
			if (thumbType === 'icon' && loadState.thumbnail === 'error') {
				for (const [, thumbId] of itemData.thumbnails) {
					if (thumbId == null || !explorerStore.newThumbnails.has(thumbId)) continue;
					// HACK: Delay removing the new thumbnail event from store
					// to avoid some weird race condition with core that prevents
					// us from accessing the new thumbnail immediately after it is created
					timeoutId = setTimeout(() => explorerStore.removeThumbnail(thumbId), 0);
					explorerStore.removeThumbnail(thumbId);
					setLoadState((state) => ({ ...state, thumbnail: 'normal' }));
					break;
				}
			}

			return () => void (timeoutId && clearTimeout(timeoutId));
		}, [itemData.thumbnails, loadState.thumbnail, thumbType]);

		const src = useMemo(() => {
			switch (thumbType) {
				case 'original':
					if (filePath && (itemData.extension !== 'pdf' || pdfViewerEnabled())) {
						if ('id' in filePath && itemData.locationId)
							return platform.getFileUrl(
								library.uuid,
								itemData.locationId,
								filePath.id
							);
						else if ('path' in filePath)
							return platform.getFileUrlByPath(filePath.path);
						else setLoadState((state) => ({ ...state, [thumbType]: 'error' }));
					}
					break;

				case 'thumbnail': {
					const thumbnail = Array.from(itemData.thumbnails.keys()).find((key) => key);
					if (thumbnail) return thumbnail;
					else setLoadState((state) => ({ ...state, [thumbType]: 'error' }));

					break;
				}
			}
		}, [
			filePath,
			itemData.extension,
			itemData.locationId,
			itemData.thumbnails,
			library.uuid,
			platform,
			thumbType
		]);

		const onError = useCallback(
			(event: Error | ErrorEvent | SyntheticEvent<Element, Event>) => {
				const rawError =
					event instanceof Error
						? event
						: ('error' in event && event.error) ||
							('message' in event && event.message) ||
							'Filetype is not supported yet';

				setLoadState((state) => {
					state = { ...state, [thumbType]: 'error' };
					props.onError?.call(
						null,
						state,
						rawError instanceof Error ? rawError : new Error(rawError)
					);
					return state;
				});
			},
			[props.onError, thumbType]
		);

		const onLoad = useCallback(() => {
			props.onLoad?.call(null, thumbType);
		}, [props.onLoad, thumbType]);

		return (
			<div
				style={{
					...(props.size
						? { maxWidth: props.size, width: props.size, height: props.size }
						: {})
				}}
				className={clsx(
					'relative flex shrink-0 items-center justify-center',
					!props.size && 'size-full',
					props.cover && 'overflow-hidden',
					props.className
				)}
			>
				<ErrorBarrier
					onError={useCallback(
						(error: Error, info: ErrorInfo) => {
							console.error('ErrorBoundary', error, info);
							onError(error);
						},
						[onError]
					)}
				>
					<Thumb
						{...props.childProps}
						ref={ref}
						src={src}
						kind={itemData.kind}
						path={filePath && 'path' in filePath ? filePath.path : null}
						frame={props.frame ?? false}
						cover={props.cover}
						isDir={itemData.isDir}
						fileId={filePath && 'id' in filePath ? filePath.id : null}
						onLoad={onLoad}
						onError={onError}
						thumbType={thumbType}
						extension={itemData.extension}
						blackBars={props.blackBars}
						className={
							typeof props.childClassName === 'function'
								? props.childClassName(thumbType)
								: props.childClassName
						}
						customIcon={itemData.customIcon as IconTypes | null}
						locationId={itemData.locationId}
						pauseVideo={props.pauseVideo ?? false}
						blackBarsSize={props.blackBarsSize}
						mediaControls={props.mediaControls ?? false}
						magnification={props.magnification ?? 1}
						frameClassName={clsx(frame.className, props.frameClassName)}
						videoExtension={
							props.extension && itemData.extension && itemData.kind === 'Video'
								? itemData.extension
								: undefined
						}
						isSidebarPreview={props.isSidebarPreview ?? false}
					/>
				</ErrorBarrier>
			</div>
		);
	})
);
