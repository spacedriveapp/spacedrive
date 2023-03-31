import * as Dialog from '@radix-ui/react-dialog';
import clsx from 'clsx';
import { XCircle } from 'phosphor-react';
import { ReactElement, useEffect, useRef, useState } from 'react';
import { useTransition } from 'react-spring';
import { animated } from 'react-spring';
import { subscribeKey } from 'valtio/utils';
import { ExplorerItem } from '~/../packages/client/src';
import { showAlertDialog } from '~/components/AlertDialog';
import { getExplorerStore } from '~/hooks/useExplorerStore';
import { usePlatform } from '~/util/Platform';
import FileThumb from './File/Thumb';
import { getExplorerItemData } from './util';

const AnimatedDialogOverlay = animated(Dialog.Overlay);
const AnimatedDialogContent = animated(Dialog.Content);
interface DialogProps extends Dialog.DialogProps {
	libraryUuid: string;
	transformOrigin?: string;
}

export function QuickPreview({ libraryUuid, transformOrigin }: DialogProps) {
	const platform = usePlatform();
	const previewItem = useRef<null | ExplorerItem>(null);
	const handleMedia = useRef<null | Promise<void>>(null);
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
				if (quickViewObject == null) return;
				setIsOpen(true);
				handleMedia.current = null;
				previewItem.current = quickViewObject;
			}),
		[explorerStore]
	);

	const onPreviewError = ({ target }: { target: EventTarget }, previewSrc: string) => {
		setIsOpen(false);
		explorerStore.quickViewObject = null;
		showAlertDialog({
			title: 'Error',
			value: 'Could not load file preview.'
		});
	};

	let preview: null | ReactElement = null;
	const previewClasses = 'relative inset-y-2/4 max-h-full max-w-full translate-y-[-50%]';
	if (previewItem.current) {
		preview = <FileThumb size={1} data={previewItem.current} className={previewClasses} />;

		const quickViewObject = previewItem.current.item;
		if (quickViewObject) {
			const locationId =
				'location_id' in quickViewObject ? quickViewObject.location_id : explorerStore.locationId;
			if (locationId) {
				const previewSrc = platform.getFileUrl(libraryUuid, locationId, quickViewObject.id);
				if (quickViewObject.extension === 'pdf') {
					if (navigator.pdfViewerEnabled)
						preview = (
							<object data={previewSrc} type="application/pdf" className="h-full w-full border-0" />
						);
				} else {
					const { kind } = getExplorerItemData(previewItem.current);
					switch (kind) {
						case 'Image':
							preview = (
								<img
									src={previewSrc}
									alt="File preview"
									onError={(err) => onPreviewError(err, previewSrc)}
									className={previewClasses}
									crossOrigin="anonymous"
								/>
							);
							break;
						case 'Audio':
							preview = (
								<>
									{preview}
									<audio
										src={previewSrc}
										onError={(err) => onPreviewError(err, previewSrc)}
										controls
										autoPlay
										className="absolute left-2/4 top-full w-full translate-y-[-150%] -translate-x-1/2"
										crossOrigin="anonymous"
									>
										<p>Audio preview is not supported.</p>
									</audio>
								</>
							);
							break;
						case 'Video':
							preview = (
								<video
									src={previewSrc}
									onError={(err) => onPreviewError(err, previewSrc)}
									controls
									autoPlay
									className={previewClasses}
									crossOrigin="anonymous"
									playsInline
								>
									<p>Video preview is not supported.</p>
								</video>
							);
							break;
					}
				}
			}
		}
	}

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
				{transitions(
					(styles, show) =>
						show && (
							<>
								<AnimatedDialogOverlay
									style={{
										opacity: styles.opacity
									}}
									className="z-49 bg-app/50 absolute inset-0 m-[1px] grid place-items-center overflow-y-auto rounded-xl"
									forceMount
								/>
								<AnimatedDialogContent
									style={styles}
									className="!pointer-events-none absolute inset-0 z-50 grid place-items-center"
									forceMount
								>
									<div className="border-app-line bg-app-box text-ink shadow-app-shade !pointer-events-auto h-5/6 w-11/12 rounded-md border">
										<nav className="flex w-full flex-row">
											<Dialog.Close className="text-ink-dull ml-2" aria-label="Close">
												<XCircle size={16} />
											</Dialog.Close>
											<Dialog.Title className="mx-auto my-1 font-bold">
												Preview -{' '}
												<span className="text-ink-dull inline-block max-w-xs truncate align-sub text-sm">
													{previewItem.current?.item?.name ?? 'Unkown Object'}
												</span>
											</Dialog.Title>
										</nav>
										<div
											className={clsx(
												'relative m-auto h-[calc(100%-2rem)] overflow-hidden',
												preview?.type === 'object' || 'w-fit'
											)}
										>
											{preview}
										</div>
									</div>
								</AnimatedDialogContent>
							</>
						)
				)}
			</Dialog.Root>
		</>
	);
}
