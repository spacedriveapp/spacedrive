import clsx from 'clsx';
import { motion } from 'framer-motion';
import { FolderSimplePlus } from 'phosphor-react';
import { useLayoutEffect, useRef, useState } from 'react';
import { Button, ButtonProps, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import { useCallbackToWatchResize } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { AddLocationDialog, openDirectoryPickerDialog } from './AddLocationDialog';

interface AddLocationButton extends ButtonProps {
	path?: string;
}

const FOLDER_ADD_ICON_SIZE = 22;

export const AddLocationButton = ({ path, className, ...props }: AddLocationButton) => {
	const platform = usePlatform();
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
		console.log(text.scrollWidth > overflow.clientWidth);

		setIsOverflowing(text.scrollWidth > overflow.clientWidth);
	}, [overflowRef, textRef]);

	return (
		<>
			<Button
				variant="dotted"
				className={clsx('w-full', className)}
				onClick={async () => {
					if (!path) {
						try {
							path = (await openDirectoryPickerDialog(platform)) ?? undefined;
						} catch (error) {
							showAlertDialog({ title: 'Error', value: String(error) });
						}
					}
					if (path)
						dialogManager.create((dp) => (
							<AddLocationDialog path={path ?? ''} {...dp} />
						));
				}}
				{...props}
			>
				{path ? (
					<div
						style={{ height: FOLDER_ADD_ICON_SIZE }}
						className="flex w-full flex-row items-end whitespace-nowrap font-mono text-sm text-ink-faint"
					>
						<FolderSimplePlus size={FOLDER_ADD_ICON_SIZE} className="shrink-0" />
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
