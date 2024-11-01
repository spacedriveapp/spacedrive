import { FolderSimplePlus } from '@phosphor-icons/react';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { ComponentProps, useRef, useState } from 'react';
import { useLibraryContext } from '@sd/client';
import { Button, dialogManager, type ButtonProps } from '@sd/ui';
import { useCallbackToWatchResize, useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { AddLocationDialog } from './AddLocationDialog';
import { openDirectoryPickerDialog } from './openDirectoryPickerDialog';

interface AddLocationButton extends ButtonProps {
	path?: string;
	onClick?: () => void;
	buttonVariant?: ComponentProps<typeof Button>['variant'];
}

export const AddLocationButton = ({
	path,
	className,
	onClick,
	buttonVariant = 'dotted',
	...props
}: AddLocationButton) => {
	const platform = usePlatform();
	const libraryId = useLibraryContext().library.uuid;

	const { t } = useLocale();

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
				variant={buttonVariant}
				className={clsx('w-full', className)}
				onClick={async () => {
					await locationDialogHandler();
					onClick?.();
				}}
				{...props}
			>
				{path ? (
					<div className="font-mono flex size-full flex-row items-end whitespace-nowrap text-sm">
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
					t('add_location')
				)}
			</Button>
		</>
	);
};
