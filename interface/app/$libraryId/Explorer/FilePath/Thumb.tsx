import { getIcon, getIconByName } from '@sd/assets/util';
import clsx from 'clsx';
import {
	forwardRef,
	HTMLAttributes,
	SyntheticEvent,
	useImperativeHandle,
	useMemo,
	useRef,
	useState
} from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { getItemFilePath, useLibraryContext, type ExplorerItem } from '@sd/client';
import { useIsDark } from '~/hooks';
import { pdfViewerEnabled } from '~/util/pdfViewer';
import { usePlatform } from '~/util/Platform';

import { useExplorerItemData } from '../useExplorerItemData';
import { Image, ImageProps } from './Image';
import LayeredFileIcon from './LayeredFileIcon';
import { Original } from './Original';
import { useFrame } from './useFrame';
import { useBlackBars, useSize } from './utils';

export interface ThumbProps {
	data: ExplorerItem;
	loadOriginal?: boolean;
	size?: number;
	cover?: boolean;
	frame?: boolean;
	onLoad?: (state: ThumbType) => void;
	onError?: (state: ThumbType, error: Error) => void;
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

type ThumbType = { variant: 'original' } | { variant: 'thumbnail' } | { variant: 'icon' };

export const FileThumb = forwardRef<HTMLImageElement, ThumbProps>((props, ref) => {
	const isDark = useIsDark();
	const platform = usePlatform();
	const frame = useFrame();

	const itemData = useExplorerItemData(props.data);
	const filePath = getItemFilePath(props.data);

	const { library } = useLibraryContext();

	const [loadState, setLoadState] = useState<{
		[K in 'original' | 'thumbnail' | 'icon']: 'notLoaded' | 'loaded' | 'error';
	}>({ original: 'notLoaded', thumbnail: 'notLoaded', icon: 'notLoaded' });

	const childClassName = 'max-h-full max-w-full object-contain';
	const frameClassName = clsx(frame.className, props.frameClassName);

	const thumbType = useMemo<ThumbType>(() => {
		const thumbType = 'thumbnail';

		if (thumbType === 'thumbnail')
			if (
				loadState.thumbnail !== 'error' &&
				itemData.hasLocalThumbnail &&
				itemData.thumbnailKey
			)
				return { variant: 'thumbnail' };

		return { variant: 'icon' };
	}, [itemData, loadState]);

	const src = useMemo(() => {
		switch (thumbType.variant) {
			case 'original':
				if (filePath && (itemData.extension !== 'pdf' || pdfViewerEnabled())) {
					if ('id' in filePath && itemData.locationId)
						return platform.getFileUrl(library.uuid, itemData.locationId, filePath.id);
					else if ('path' in filePath) return platform.getFileUrlByPath(filePath.path);
				}
				break;

			case 'thumbnail':
				if (itemData.thumbnailKey)
					return platform.getThumbnailUrlByThumbKey(itemData.thumbnailKey);

				break;
			case 'icon':
				if (itemData.customIcon) return getIconByName(itemData.customIcon as any, isDark);

				return getIcon(
					// itemData.isDir || parent?.type === 'Node' ? 'Folder' :
					itemData.kind,
					isDark,
					itemData.extension,
					itemData.isDir
				);
		}
	}, [filePath, isDark, library.uuid, itemData, platform, thumbType]);

	const onLoad = (s: 'original' | 'thumbnail' | 'icon') => {
		setLoadState((state) => ({ ...state, [s]: 'loaded' }));
		props.onLoad?.call(null, thumbType);
	};

	const onError = (
		s: 'original' | 'thumbnail' | 'icon',
		event: ErrorEvent | SyntheticEvent<Element, Event>
	) => {
		setLoadState((state) => ({ ...state, [s]: 'error' }));

		const rawError =
			('error' in event && event.error) ||
			('message' in event && event.message) ||
			'Filetype is not supported yet';

		props.onError?.call(
			null,
			thumbType,
			rawError instanceof Error ? rawError : new Error(rawError)
		);
	};

	const _childClassName =
		typeof props.childClassName === 'function'
			? props.childClassName(thumbType)
			: props.childClassName;

	const className = clsx(childClassName, _childClassName);

	const thumbnail = (() => {
		if (!src) return <></>;

		switch (thumbType.variant) {
			case 'thumbnail':
				return (
					<Thumbnail
						{...props.childProps}
						ref={ref}
						src={src}
						cover={props.cover}
						onLoad={() => onLoad('thumbnail')}
						onError={(e) => onError('thumbnail', e)}
						decoding={props.size ? 'async' : 'sync'}
						className={clsx(
							props.cover
								? [
										'min-h-full min-w-full object-cover object-center',
										_childClassName
									]
								: className,
							props.frame && !(itemData.kind === 'Video' && props.blackBars)
								? frameClassName
								: null
						)}
						crossOrigin="anonymous" // Here it is ok, because it is not a react attr
						blackBars={props.blackBars && itemData.kind === 'Video' && !props.cover}
						blackBarsSize={props.blackBarsSize}
						extension={
							props.extension && itemData.extension && itemData.kind === 'Video'
								? itemData.extension
								: undefined
						}
					/>
				);

			case 'icon':
				return (
					<LayeredFileIcon
						{...props.childProps}
						ref={ref}
						src={src}
						kind={itemData.kind}
						extension={itemData.extension}
						onLoad={() => onLoad('icon')}
						onError={(e) => onError('icon', e)}
						decoding={props.size ? 'async' : 'sync'}
						className={className}
						draggable={false}
					/>
				);
			default:
				return <></>;
		}
	})();

	return (
		<div
			key={thumbType.variant}
			style={{
				...(props.size
					? { maxWidth: props.size, width: props.size, height: props.size }
					: {})
			}}
			className={clsx(
				'relative flex shrink-0 items-center justify-center',
				// !loaded && 'invisible',
				!props.size && 'size-full',
				props.cover && 'overflow-hidden',
				props.className
			)}
		>
			{props.loadOriginal ? (
				<ErrorBoundary fallback={thumbnail}>
					<Original
						onLoad={() => onLoad('original')}
						onError={(e) => onError('original', e)}
						filePath={filePath}
						className={className}
						frameClassName={frameClassName}
						itemData={itemData}
						isDark={isDark}
						childClassName={childClassName}
						size={props.size}
						magnification={props.magnification}
						mediaControls={props.mediaControls}
						frame={props.frame}
						isSidebarPreview={props.isSidebarPreview}
						pauseVideo={props.pauseVideo}
						blackBars={props.blackBars}
						blackBarsSize={props.blackBarsSize}
					/>
				</ErrorBoundary>
			) : (
				thumbnail
			)}
		</div>
	);
});

interface ThumbnailProps extends Omit<ImageProps, 'blackBarsStyle' | 'size'> {
	cover?: boolean;
	blackBars?: boolean;
	blackBarsSize?: number;
	extension?: string;
}

const Thumbnail = forwardRef<HTMLImageElement, ThumbnailProps>(
	({ blackBars, blackBarsSize, extension, cover, className, style, ...props }, _ref) => {
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
					size={size}
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
);
