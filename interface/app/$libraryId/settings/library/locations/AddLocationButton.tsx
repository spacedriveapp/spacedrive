import clsx from 'clsx';
import { motion } from 'framer-motion';
import { FolderSimplePlus } from 'phosphor-react';
import { useRef, useState } from 'react';
import { Button, type ButtonProps, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useCallbackToWatchResize } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { AddLocationDialog, openDirectoryPickerDialog } from './AddLocationDialog';

interface AddLocationButton extends ButtonProps {
	path?: string;
}

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
