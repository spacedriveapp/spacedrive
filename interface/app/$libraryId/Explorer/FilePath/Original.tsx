import { getIcon, iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import {
	SyntheticEvent,
	useEffect,
	useMemo,
	useRef,
	useState,
	type VideoHTMLAttributes
} from 'react';
import { getItemFilePath, useLibraryContext } from '@sd/client';
import i18n from '~/app/I18n';
import { PDFViewer, TextViewer } from '~/components';
import { useLocale } from '~/hooks';
import { pdfViewerEnabled } from '~/util/pdfViewer';
import { usePlatform } from '~/util/Platform';

import { useExplorerContext } from '../Context';
import { explorerStore } from '../store';
import { ExplorerItemData } from '../useExplorerItemData';
import { Image } from './Image';
import { useBlackBars, useSize } from './utils';

interface OriginalRendererProps {
	src: string;
	className: string;
	frameClassName: string;
	itemData: ExplorerItemData;
	isDark: boolean;
	childClassName?: string;
	size?: number;
	magnification?: number;
	mediaControls?: boolean;
	frame?: boolean;
	isSidebarPreview?: boolean;
	pauseVideo?: boolean;
	blackBars?: boolean;
	blackBarsSize?: number;
	onLoad?(): void;
	onError?(e: ErrorEvent | SyntheticEvent<Element, Event>): void;
}

export function Original({
	itemData,
	filePath,
	...props
}: Omit<OriginalRendererProps, 'src'> & {
	filePath: ReturnType<typeof getItemFilePath>;
}) {
	const [error, setError] = useState(false);
	if (error) throw new Error('onError');

	const Renderer = useMemo(() => {
		const kind = originalRendererKind(itemData);
		return ORIGINAL_RENDERERS[kind];
	}, [itemData]);

	if (!Renderer) throw new Error('no renderer!');

	const platform = usePlatform();
	const { library } = useLibraryContext();
	const { parent } = useExplorerContext();

	const src = useMemo(() => {
		const locationId =
			itemData.locationId ?? (parent?.type === 'Location' ? parent.location.id : null);

		if (filePath && (itemData.extension !== 'pdf' || pdfViewerEnabled())) {
			if ('id' in filePath && locationId)
				return platform.getFileUrl(library.uuid, locationId, filePath.id);
			else if ('path' in filePath) return platform.getFileUrlByPath(filePath.path);
		}
	}, [itemData, filePath, library.uuid, parent, platform]);

	if (src === undefined) throw new Error('no src!');

	return <Renderer src={src} itemData={itemData} onError={() => setError(true)} {...props} />;
}

const TEXT_RENDERER: OriginalRenderer = (props) => (
	<TextViewer
		src={props.src}
		onLoad={props.onLoad}
		onError={props.onError}
		className={clsx(
			'textviewer-scroll size-full overflow-y-auto whitespace-pre-wrap break-words px-4 font-mono',
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
			className={clsx('size-full', props.className, props.frame && props.frameClassName)}
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
					<p>{i18n.t('audio_preview_not_supported')}</p>
				</audio>
			)}
		</>
	),
	Image: (props) => {
		const ref = useRef<HTMLImageElement>(null);
		const size = useSize(ref);

		return (
			<div className="custom-scroll quick-preview-images-scroll flex size-full justify-center transition-all">
				<Image
					ref={ref}
					src={props.src}
					size={size}
					style={{ transform: `scale(${props.magnification})` }}
					onLoad={props.onLoad}
					onError={props.onError}
					decoding={props.size ? 'async' : 'sync'}
					className={clsx(
						props.className,
						props.frameClassName,
						'origin-center transition-transform'
					)}
					crossOrigin="anonymous" // Here it is ok, because it is not a react attr
				/>
			</div>
		);
	}
};

interface VideoProps extends VideoHTMLAttributes<HTMLVideoElement> {
	paused?: boolean;
	blackBars?: boolean;
	blackBarsSize?: number;
}

const Video = ({ paused, blackBars, blackBarsSize, className, ...props }: VideoProps) => {
	const { t } = useLocale();

	const ref = useRef<HTMLVideoElement>(null);

	const size = useSize(ref);

	const { style: blackBarsStyle } = useBlackBars(ref, size, {
		size: blackBarsSize,
		disabled: !blackBars
	});

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
				explorerStore.mediaPlayerVolume = video.volume;
			}}
			onCanPlay={(e) => {
				const video = e.target as HTMLVideoElement;
				// Why not use the element's attribute? Because React...
				// https://github.com/facebook/react/issues/10389
				video.loop = !props.controls;
				video.muted = !props.controls;
				video.volume = explorerStore.mediaPlayerVolume;
			}}
			playsInline
			draggable={false}
			style={{ ...blackBarsStyle }}
			className={clsx(blackBars && size.width === 0 && 'invisible', className)}
			{...props}
			key={props.src}
			controls={false}
			onTimeUpdate={(e) => {
				const video = e.target as HTMLVideoElement;
				if (video.currentTime > 0) {
					video.controls = props.controls ?? true;
				}
			}}
		>
			<p>{t('video_preview_not_supported')}</p>
		</video>
	);
};
