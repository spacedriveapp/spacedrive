import clsx from 'clsx';
import { motion } from 'framer-motion';
import { FolderSimplePlus } from 'phosphor-react';
import { Button, ButtonProps, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
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
					<div className="flex h-full w-full flex-row items-center whitespace-nowrap font-mono text-xs text-ink-faint">
						<FolderSimplePlus size={18} className="shrink-0" />
						<div className="ml-1 overflow-hidden">
							<motion.span
								animate={{ x: ['0%', '100%', '0%'] }}
								className="inline-block w-full"
								transition={{ ...transition }}
							>
								<motion.span
									animate={{ x: ['0%', '-100%', '0%'] }}
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
