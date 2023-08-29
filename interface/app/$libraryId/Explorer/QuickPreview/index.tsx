import * as Dialog from '@radix-ui/react-dialog';
import { animated, useTransition } from '@react-spring/web';
import { X } from 'phosphor-react';
import { useEffect, useState } from 'react';
import { subscribeKey } from 'valtio/utils';
import { type ExplorerItem, getExplorerItemData } from '@sd/client';
import { Button } from '@sd/ui';
import { FileThumb } from '../FilePath/Thumb';
import { getExplorerStore } from '../store';

const AnimatedDialogOverlay = animated(Dialog.Overlay);
const AnimatedDialogContent = animated(Dialog.Content);

export interface QuickPreviewProps extends Dialog.DialogProps {
	transformOrigin?: string;
}

export function QuickPreview({ transformOrigin }: QuickPreviewProps) {
	const [explorerItem, setExplorerItem] = useState<null | ExplorerItem>(null);
	const explorerStore = getExplorerStore();
	const [isOpen, setIsOpen] = useState<boolean>(false);

	/**
	 * The useEffect hook with subscribe is used here, instead of useExplorerStore, because when
	 * explorerStore.quickViewObject is set to null the component will not close immediately.
	 * Instead, it will enter the beginning of the close transition and it must continue to display
	 * content for a few more seconds due to the ongoing animation. To handle this, the open state
	 * is decoupled from the store state, by assigning references to the required store properties
	 * to render the component in the subscribe callback.
	 */
	useEffect(
		() =>
			subscribeKey(explorerStore, 'quickViewObject', () => {
				const { quickViewObject } = explorerStore;
				if (quickViewObject != null) {
					setIsOpen(true);
					setExplorerItem(quickViewObject);
				} else {
					setIsOpen(false);
				}
			}),
		[explorerStore]
	);

	const transitions = useTransition(isOpen, {
		from: {
			opacity: 0,
			transform: `translateY(20px) scale(0.9)`,
			transformOrigin: transformOrigin || 'center top'
		},
		enter: { opacity: 1, transform: `translateY(0px) scale(1)` },
		leave: { opacity: 0, transform: `translateY(40px) scale(0.9)` },
		config: { mass: 0.2, tension: 300, friction: 20, bounce: 0 }
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
					if (!show || explorerItem == null) return null;

					const { name } = getExplorerItemData(explorerItem);

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
									<div className="!pointer-events-auto flex h-5/6 max-h-screen w-11/12 flex-col overflow-y-auto rounded-md border border-app-line bg-app-box text-ink shadow-app-shade">
										<nav className="relative flex w-full flex-row">
											<Dialog.Close
												asChild
												className="absolute m-2"
												aria-label="Close"
											>
												<Button
													size="icon"
													variant="outline"
													className="flex flex-row"
												>
													<X
														weight="bold"
														className=" h-3 w-3 text-ink-faint"
													/>
													<span className="ml-1 text-tiny font-medium text-ink-faint">
														ESC
													</span>
												</Button>
											</Dialog.Close>
											<Dialog.Title className="mx-auto my-2 font-bold">
												Preview -{' '}
												<span className="inline-block max-w-xs truncate align-sub text-sm text-ink-dull">
													{name || 'Unknown Object'}
												</span>
											</Dialog.Title>
										</nav>
										<div className="flex h-full w-full shrink items-center justify-center overflow-hidden">
											<FileThumb
												data={explorerItem}
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
