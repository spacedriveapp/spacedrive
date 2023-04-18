import clsx from 'clsx';
import { Button, ButtonProps, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import { usePlatform } from '~/util/Platform';
import { AddLocationDialog, openDirectoryPickerDialog } from './AddLocationDialog';

export const AddLocationButton = ({ className, ...props }: ButtonProps) => {
	const platform = usePlatform();

	return (
		<>
			<Button
				variant="dotted"
				className={clsx('w-full', className)}
				onClick={() =>
					openDirectoryPickerDialog(platform)
						.then((path) => {
							if (path !== '')
								dialogManager.create((dp) => (
									<AddLocationDialog path={path ?? ''} {...dp} />
								));
						})
						.catch((error) => showAlertDialog({ title: 'Error', value: String(error) }))
				}
				{...props}
			>
				Add Location
			</Button>
		</>
	);
};
