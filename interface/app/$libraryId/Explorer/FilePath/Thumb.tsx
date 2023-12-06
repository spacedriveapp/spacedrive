import { getIcon, getIconByName, iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import {
	forwardRef,
	HTMLAttributes,
	SyntheticEvent,
	useEffect,
	useImperativeHandle,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type CSSProperties,
	type ImgHTMLAttributes,
	type RefObject,
	type VideoHTMLAttributes
} from 'react';
import { getItemFilePath, useLibraryContext, type ExplorerItem } from '@sd/client';
import { PDFViewer, TextViewer } from '~/components';
import { useCallbackToWatchResize, useIsDark } from '~/hooks';
import { pdfViewerEnabled } from '~/util/pdfViewer';
import { usePlatform } from '~/util/Platform';

import { getExplorerStore } from '../store';
import { ExplorerItemData, useExplorerItemData } from '../util';
import LayeredFileIcon from './LayeredFileIcon';
import classes from './Thumb.module.scss';

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
}

type ThumbType =
	| { variant: 'original'; renderer: OriginalRenderer }
	| { variant: 'thumbnail' }
	| { variant: 'icon' };

export const FileThumb = forwardRef<HTMLImageElement, ThumbProps>((props, ref) => {
	const isDark = useIsDark();
	const platform = usePlatform();

	const itemData = useExplorerItemData(props.data);
	const filePath = getItemFilePath(props.data);

	const { library } = useLibraryContext();

	const [loadState, setLoadState] = useState<'notLoaded' | 'loaded' | 'error'>('notLoaded');

	const childClassName = 'max-h-full max-w-full object-contain';
	const frameClassName = clsx(
		'rounded-sm border-2 border-app-line bg-app-darkBox',
		props.frameClassName,
		isDark ? classes.checkers : classes.checkersLight
	);

	const thumbType = useMemo<ThumbType>(() => {
		const expectedThumbType: ThumbType = (() => {
			if (props.loadOriginal) {
				if (loadState === 'error' && !itemData.hasLocalThumbnail)
					return { variant: 'icon' };

				const kind = originalRendererKind(itemData);
				const renderer = ORIGINAL_RENDERERS[kind];

				if (renderer) return { variant: 'original', renderer };

				return { variant: 'thumbnail' };
			} else if (itemData.hasLocalThumbnail) return { variant: 'thumbnail' };
			else return { variant: 'icon' };
		})();

		if (expectedThumbType.variant === 'thumbnail' && itemData.thumbnailKey.length === 0)
			return { variant: 'icon' };

		return expectedThumbType;
	}, [props.loadOriginal, itemData, loadState]);

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
				if (itemData.thumbnailKey.length > 0)
					return platform.getThumbnailUrlByThumbKey(itemData.thumbnailKey);

				break;
			case 'icon':
				if (itemData.customIcon) return getIconByName(itemData.customIcon as any);

				return getIcon(
					// itemData.isDir || parent?.type === 'Node' ? 'Folder' :
					itemData.kind,
					isDark,
					itemData.extension,
					itemData.isDir
				);
		}
	}, [filePath, isDark, library.uuid, itemData, platform, thumbType]);

	const onLoad = () => {
		setLoadState('loaded');
		props.onLoad?.call(null, thumbType);
	};

	const onError = (event: ErrorEvent | SyntheticEvent<Element, Event>) => {
		setLoadState('error');

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

	// useLayoutEffect is required to ensure the thumbType is always updated before the onError listener can execute,
	// thus avoiding improper thumb types changes
	useLayoutEffect(() => {
		// Reset src when item changes, to allow detection of yet not updated src
		setLoadState('notLoaded');
	}, [src, thumbType]);

	return (
		<div
			style={{
				...(props.size
					? { maxWidth: props.size, width: props.size, height: props.size }
					: {})
			}}
			className={clsx(
				'relative flex shrink-0 items-center justify-center',
				// !loaded && 'invisible',
				!props.size && 'h-full w-full',
				props.cover && 'overflow-hidden',
				props.className
			)}
		>
			{(() => {
				if (!src) return;

				const _childClassName =
					typeof props.childClassName === 'function'
						? props.childClassName(thumbType)
						: props.childClassName;

				const className = clsx(childClassName, _childClassName);

				switch (thumbType.variant) {
					case 'original':
						return thumbType.renderer({
							src,
							className,
							frameClassName,
							itemData,
							isDark,
							childClassName,
							onLoad,
							onError,
							size: props.size,
							mediaControls: props.mediaControls,
							frame: props.frame,
							isSidebarPreview: props.isSidebarPreview,
							pauseVideo: props.pauseVideo,
							blackBars: props.blackBars,
							blackBarsSize: props.blackBarsSize
						});

					// eslint-disable-next-line no-fallthrough
					case 'thumbnail':
						return (
							<Thumbnail
								{...props.childProps}
								ref={ref}
								src={src}
								cover={props.cover}
								onLoad={onLoad}
								onError={onError}
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
								blackBars={
									props.blackBars && itemData.kind === 'Video' && !props.cover
								}
								blackBarsSize={props.blackBarsSize}
								extension={
									props.extension &&
									itemData.extension &&
									itemData.kind === 'Video'
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
								onLoad={onLoad}
								onError={() => setLoadState('notLoaded')}
								decoding={props.size ? 'async' : 'sync'}
								className={className}
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
	blackBars?: boolean;
	blackBarsSize?: number;
	extension?: string;
}

const Thumbnail = forwardRef<HTMLImageElement, ThumbnailProps>(
	(
		{ crossOrigin, blackBars, blackBarsSize, extension, cover, className, style, ...props },
		_ref
	) => {
		const ref = useRef<HTMLImageElement>(null);
		useImperativeHandle<HTMLImageElement | null, HTMLImageElement | null>(
			_ref,
			() => ref.current
		);

		const size = useSize(ref);

		const { style: blackBarsStyle } = useBlackBars(size, blackBarsSize);

		return (
			<>
				<img
					// Order matter for crossOrigin attr
					// https://github.com/facebook/react/issues/14035#issuecomment-642227899
					{...(crossOrigin ? { crossOrigin } : {})}
					{...props}
					ref={ref}
					draggable={false}
					style={{ ...style, ...(blackBars ? blackBarsStyle : {}) }}
					className={clsx(blackBars && size.width === 0 && 'invisible', className)}
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

interface OriginalRendererProps {
	src: string;
	className: string;
	frameClassName: string;
	itemData: ExplorerItemData;
	isDark: boolean;
	childClassName?: string;
	size?: number;
	mediaControls?: boolean;
	frame?: boolean;
	isSidebarPreview?: boolean;
	pauseVideo?: boolean;
	blackBars?: boolean;
	blackBarsSize?: number;
	onLoad?(): void;
	onError?(e: ErrorEvent | SyntheticEvent<Element, Event>): void;
}

const TEXT_RENDERER: OriginalRenderer = (props) => (
	<TextViewer
		src={props.src}
		onLoad={props.onLoad}
		onError={props.onError}
		className={clsx(
			'textviewer-scroll h-full w-full overflow-y-auto whitespace-pre-wrap break-words px-4 font-mono',
			!props.mediaControls ? 'overflow-hidden' : 'overflow-auto',
			props.className,
			props.frame && [props.frameClassName, '!bg-none p-2']
		)}
		codeExtension={
			((props.itemData.kind === 'Code' || props.itemData.kind === 'Config') &&
				props.itemData.extension) ||
			''
		}
		isSidebarPreview={props.isSidebarPreview}
	/>
);

type OriginalRenderer = (props: OriginalRendererProps) => JSX.Element;

function originalRendererKind(itemData: ExplorerItemData) {
	return itemData.extension === 'pdf' ? 'PDF' : itemData.kind;
}

type OriginalRendererKind = ReturnType<typeof originalRendererKind>;

const ORIGINAL_RENDERERS: {
	[K in OriginalRendererKind]?: OriginalRenderer;
} = {
	PDF: (props) => (
		<PDFViewer
			src={props.src}
			onLoad={props.onLoad}
			onError={props.onError}
			className={clsx('h-full w-full', props.className, props.frame && props.frameClassName)}
			crossOrigin="anonymous" // Here it is ok, because it is not a react attr
		/>
	),
	Text: TEXT_RENDERER,
	Code: TEXT_RENDERER,
	Config: TEXT_RENDERER,
	Video: (props) => (
		<Video
			src={props.src}
			onLoadedData={props.onLoad}
			onError={props.onError}
			paused={props.pauseVideo}
			controls={props.mediaControls}
			blackBars={props.blackBars}
			blackBarsSize={props.blackBarsSize}
			className={clsx(
				props.className,
				props.frame && !props.blackBars && props.frameClassName
			)}
		/>
	),
	Audio: (props) => (
		<>
			<img
				src={getIcon(iconNames.Audio, props.isDark, props.itemData.extension)}
				onLoad={props.onLoad}
				decoding={props.size ? 'async' : 'sync'}
				className={props.childClassName}
				draggable={false}
			/>
			{props.mediaControls && (
				<audio
					// Order matter for crossOrigin attr
					crossOrigin="anonymous"
					src={props.src}
					onError={props.onError}
					controls
					autoPlay
					className="absolute left-2/4 top-full w-full -translate-x-1/2 translate-y-[-150%]"
				>
					<p>Audio preview is not supported.</p>
				</audio>
			)}
		</>
	)
};

interface VideoProps extends VideoHTMLAttributes<HTMLVideoElement> {
	paused?: boolean;
	blackBars?: boolean;
	blackBarsSize?: number;
}

const Video = ({ paused, blackBars, blackBarsSize, className, ...props }: VideoProps) => {
	const ref = useRef<HTMLVideoElement>(null);

	const size = useSize(ref);
	const { style: blackBarsStyle } = useBlackBars(size, blackBarsSize);

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
			style={{ ...(blackBars ? blackBarsStyle : {}) }}
			className={clsx(blackBars && size.width === 0 && 'invisible', className)}
			{...props}
		>
			<p>Video preview is not supported.</p>
		</video>
	);
};

const useSize = (ref: RefObject<Element>) => {
	const [size, setSize] = useState({ width: 0, height: 0 });

	useCallbackToWatchResize(({ width, height }) => setSize({ width, height }), [], ref);

	return size;
};

const useBlackBars = (videoSize: { width: number; height: number }, blackBarsSize?: number) => {
	return useMemo(() => {
		const { width, height } = videoSize;

		const orientation = height > width ? 'vertical' : 'horizontal';

		const barSize =
			blackBarsSize ||
			Math.floor(Math.ceil(orientation === 'vertical' ? height : width) / 10);

		const xBarSize = orientation === 'vertical' ? barSize : 0;
		const yBarSize = orientation === 'horizontal' ? barSize : 0;

		return {
			size: {
				x: xBarSize,
				y: yBarSize
			},
			style: {
				borderLeftWidth: xBarSize,
				borderRightWidth: xBarSize,
				borderTopWidth: yBarSize,
				borderBottomWidth: yBarSize,
				borderColor: 'black',
				borderRadius: 4
			} satisfies CSSProperties
		};
	}, [videoSize, blackBarsSize]);
};
