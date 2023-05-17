import * as Dialog from '@radix-ui/react-dialog';
import { animated, useTransition } from '@react-spring/web';
import { XCircle } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import { subscribeKey } from 'valtio/utils';
import { ExplorerItem } from '@sd/client';
import { getExplorerStore } from '~/hooks';
import FileThumb from './File/Thumb';

const AnimatedDialogOverlay = animated(Dialog.Overlay);
const AnimatedDialogContent = animated(Dialog.Content);

export interface QuickPreviewProps extends Dialog.DialogProps {
	transformOrigin?: string;
}

export function QuickPreview({ transformOrigin }: QuickPreviewProps) {
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
										<div className="flex h-full w-full shrink items-center justify-center overflow-hidden">
											<FileThumb
												size={0}
												data={explorerItem.current}
												loadOriginal
												mediaControls
											/>
										</div>
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
