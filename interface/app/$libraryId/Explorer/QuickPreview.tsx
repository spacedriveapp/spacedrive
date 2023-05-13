import * as Dialog from '@radix-ui/react-dialog';
import { animated, useTransition } from '@react-spring/web';
import clsx from 'clsx';
import { XCircle } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import { subscribeKey } from 'valtio/utils';
import { ExplorerItem } from '~/../packages/client/src';
import { showAlertDialog } from '~/components/AlertDialog';
import { getExplorerStore } from '~/hooks/useExplorerStore';
import { usePlatform } from '~/util/Platform';
import FileThumb from './File/Thumb';
import { getExplorerItemData } from './util';

const AnimatedDialogOverlay = animated(Dialog.Overlay);
const AnimatedDialogContent = animated(Dialog.Content);

export interface QuickPreviewProps extends Dialog.DialogProps {
	libraryUuid: string;
	transformOrigin?: string;
}
interface FilePreviewProps {
	src: string;
	kind: null | string;
	onError: () => void;
	explorerItem: ExplorerItem;
}

/**
 * Check if webview can display PDFs
 * https://developer.mozilla.org/en-US/docs/Web/API/Navigator/pdfViewerEnabled
 * https://developer.mozilla.org/en-US/docs/Web/API/Navigator/mimeTypes
 * https://developer.mozilla.org/en-US/docs/Web/API/Navigator/plugins
 */
const pdfViewerEnabled = () => {
	// pdfViewerEnabled is quite new, Safari only started supporting it in march 2023
	// https://caniuse.com/?search=pdfViewerEnabled
	if ('pdfViewerEnabled' in navigator && navigator.pdfViewerEnabled) return true;

	// This is deprecated, but should be supported on all browsers/webviews
	// https://caniuse.com/mdn-api_navigator_mimetypes
	if (navigator.mimeTypes) {
		if ('application/pdf' in navigator.mimeTypes)
			return (navigator.mimeTypes['application/pdf'] as null | MimeType)?.enabledPlugin;
		if ('text/pdf' in navigator.mimeTypes)
			return (navigator.mimeTypes['text/pdf'] as null | MimeType)?.enabledPlugin;
	}

	// Last ditch effort
	// https://caniuse.com/mdn-api_navigator_plugins
	return 'PDF Viewer' in navigator.plugins;
};

function FilePreview({ explorerItem, kind, src, onError }: FilePreviewProps) {
	const className = clsx('object-contain');
	const fileThumb = <FileThumb size={0} data={explorerItem} cover className={className} />;
	switch (kind) {
		case 'PDF':
			return <object data={src} type="application/pdf" className="h-full w-full border-0" />;
		case 'Image':
			return (
				<img
					src={src}
					alt="File preview"
					onError={onError}
					className={className}
					crossOrigin="anonymous"
				/>
			);
		case 'Audio':
			return (
				<>
					{fileThumb}
					<audio
						src={src}
						onError={onError}
						controls
						autoPlay
						className="absolute left-2/4 top-full w-full -translate-x-1/2 translate-y-[-150%]"
						crossOrigin="anonymous"
					>
						<p>Audio preview is not supported.</p>
					</audio>
				</>
			);
		case 'Video':
			return (
				<video
					src={src}
					onError={onError}
					controls
					autoPlay
					className={className}
					crossOrigin="anonymous"
					playsInline
				>
					<p>Video preview is not supported.</p>
				</video>
			);
		default:
			return fileThumb;
	}
}

export function QuickPreview({ libraryUuid, transformOrigin }: QuickPreviewProps) {
	const platform = usePlatform();
	const explorerItem = useRef<null | ExplorerItem>(null);
	const explorerStore = getExplorerStore();
	const [isOpen, setIsOpen] = useState<boolean>(false);

	/**
	 * The useEffect hook with subscribe is used here, instead of useExplorerStore, because when
	 * explorerStore.quickViewObject is set to null the component will not close immediately.
	 * Instead, it will enter the beginning of the close transition and it must continue to display
	 * content for a few more seconds due to the ongoing animation. To handle this, the open state
	 * is decoupled from the store state, by assinging references to the required store properties
	 * to render the component in the subscribe callback.
	 */
	useEffect(
		() =>
			subscribeKey(explorerStore, 'quickViewObject', () => {
				const { quickViewObject } = explorerStore;
				if (quickViewObject != null) {
					setIsOpen(true);
					explorerItem.current = quickViewObject;
				}
			}),
		[explorerStore]
	);

	const onPreviewError = () => {
		setIsOpen(false);
		explorerStore.quickViewObject = null;
		showAlertDialog({
			title: 'Error',
			value: 'Could not load file preview.'
		});
	};

	const transitions = useTransition(isOpen, {
		from: {
			opacity: 0,
			transform: `translateY(20px)`,
			transformOrigin: transformOrigin || 'bottom'
		},
		enter: { opacity: 1, transform: `translateY(0px)` },
		leave: { opacity: 0, transform: `translateY(20px)` },
		config: { mass: 0.4, tension: 200, friction: 10, bounce: 0 }
	});

	return (
		<>
			<Dialog.Root
				open={isOpen}
				onOpenChange={(open) => {
					setIsOpen(open);
					if (!open) explorerStore.quickViewObject = null;
				}}
			>
				{transitions((styles, show) => {
					if (!show || explorerItem.current == null) return null;

					const { item } = explorerItem.current;
					const locationId =
						'location_id' in item ? item.location_id : explorerStore.locationId;
					if (locationId == null) {
						onPreviewError();
						return null;
					}

					const { kind, extension } = getExplorerItemData(explorerItem.current);
					const preview = (
						<FilePreview
							src={platform.getFileUrl(libraryUuid, locationId, item.id)}
							kind={extension === 'pdf' && pdfViewerEnabled() ? 'PDF' : kind}
							onError={onPreviewError}
							explorerItem={explorerItem.current}
						/>
					);

					return (
						<>
							<Dialog.Portal forceMount>
								<AnimatedDialogOverlay
									style={{
										opacity: styles.opacity
									}}
									className="z-49 absolute inset-0 m-[1px] grid place-items-center overflow-y-auto rounded-xl bg-app/50"
								/>
								<AnimatedDialogContent
									style={styles}
									className="!pointer-events-none absolute inset-0 z-50 grid h-screen place-items-center"
								>
									<div className="!pointer-events-auto flex h-5/6 max-h-screen w-11/12 flex-col rounded-md border border-app-line bg-app-box text-ink shadow-app-shade">
										<nav className="flex w-full flex-row">
											<Dialog.Close
												className="ml-2 text-ink-dull"
												aria-label="Close"
											>
												<XCircle size={16} />
											</Dialog.Close>
											<Dialog.Title className="mx-auto my-1 font-bold">
												Preview -{' '}
												<span className="inline-block max-w-xs truncate align-sub text-sm text-ink-dull">
													{'name' in item && item.name
														? item.name
														: 'Unkown Object'}
												</span>
											</Dialog.Title>
										</nav>
										<div className="flex shrink overflow-hidden">{preview}</div>
									</div>
								</AnimatedDialogContent>
							</Dialog.Portal>
						</>
					);
				})}
			</Dialog.Root>
		</>
	);
}
