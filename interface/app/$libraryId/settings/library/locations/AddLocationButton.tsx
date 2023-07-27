import clsx from 'clsx';
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
					<div className="flex h-full w-full flex-row items-center">
						<FolderSimplePlus size={18} className="shrink-0" />
						<span
							dir="rtl"
							className="text-crop shink mx-1 mr-2 flex h-full overflow-hidden whitespace-nowrap text-center font-mono text-xs font-medium text-ink-faint"
						>
							{path}
						</span>
					</div>
				) : (
					'Add Location'
				)}
			</Button>
		</>
	);
};
