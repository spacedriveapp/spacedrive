import { FolderSimplePlus } from '@phosphor-icons/react';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { useRef, useState } from 'react';
import { useLibraryContext } from '@sd/client';
import { Button, dialogManager, type ButtonProps } from '@sd/ui';
import { getExplorerStore } from '~/app/$libraryId/Explorer/store';
import { useCallbackToWatchResize, useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { AddLocationDialog } from './AddLocationDialog';
import { openDirectoryPickerDialog } from './openDirectoryPickerDialog';

interface AddLocationButton extends ButtonProps {
	path?: string;
	onClick?: () => void;
}

export const AddLocationButton = ({ path, className, onClick, ...props }: AddLocationButton) => {
	const platform = usePlatform();
	const libraryId = useLibraryContext().library.uuid;
	const fdaPermissions = usePlatform().hasFda;
	const os = useOperatingSystem();

	const transition = {
		type: 'keyframes',
		ease: 'easeInOut',
		repeat: Infinity,
		duration: 5
	};

	const textRef = useRef<HTMLSpanElement>(null);
	const overflowRef = useRef<HTMLSpanElement>(null);
	const [isOverflowing, setIsOverflowing] = useState(false);

	useCallbackToWatchResize(() => {
		const text = textRef.current;
		const overflow = overflowRef.current;

		if (!(text && overflow)) return;

		setIsOverflowing(text.scrollWidth > overflow.clientWidth);
	}, [overflowRef, textRef]);

	const locationDialogHandler = async () => {
		if (!path) {
			path = (await openDirectoryPickerDialog(platform)) ?? undefined;
		}
		// Remember `path` will be `undefined` on web cause the user has to provide it in the modal
		if (path !== '')
			dialogManager.create((dp) => (
				<AddLocationDialog path={path ?? ''} libraryId={libraryId} {...dp} />
			));
	};

	return (
		<>
			<Button
				variant="dotted"
				className={clsx('w-full', className)}
				onClick={async () => {
					if (os === 'macOS') {
						const permissions = await fdaPermissions?.(); //needs to be awaited for promise to resolve
						if (permissions) {
							await locationDialogHandler();
						} else if (!permissions) {
							getExplorerStore().showFda = true;
						}
					} else {
						await locationDialogHandler();
					}
					onClick?.();
				}}
				{...props}
			>
				{path ? (
					<div className="flex h-full w-full flex-row items-end whitespace-nowrap font-mono text-sm">
						<FolderSimplePlus size={22} className="shrink-0" />
						<div className="ml-1 overflow-hidden">
							<motion.span
								ref={overflowRef}
								animate={isOverflowing && { x: ['0%', '100%', '0%'] }}
								className="inline-block w-full"
								transition={{ ...transition }}
							>
								<motion.span
									ref={textRef}
									animate={isOverflowing && { x: ['0%', '-100%', '0%'] }}
									className="inline-block w-auto"
									transition={{ ...transition }}
								>
									{path}
								</motion.span>
							</motion.span>
						</div>
					</div>
				) : (
					'Add Location'
				)}
			</Button>
		</>
	);
};
