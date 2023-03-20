import { Button, ButtonProps, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import { usePlatform } from '~/util/Platform';
import { AddLocationDialog, openDirectoryPickerDialog } from './AddLocationDialog';

export const AddLocationButton = (props: ButtonProps) => {
	const platform = usePlatform();

	return (
		<>
			<Button
				{...props}
				onClick={() =>
					openDirectoryPickerDialog(platform)
						.then((path) => {
							if (path) dialogManager.create((dp) => <AddLocationDialog path={path} {...dp} />);
						})
						.catch((error) =>
							showAlertDialog({
								title: 'Error',
								value: String(error)
							})
						)
				}
			>
				Add Location
			</Button>
		</>
	);
};
