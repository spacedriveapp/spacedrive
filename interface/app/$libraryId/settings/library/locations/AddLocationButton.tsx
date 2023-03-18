import { useLibraryMutation, useLibraryQuery, usePlausibleEvent } from '@sd/client';
import { Button, ButtonProps, dialogManager } from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import { usePlatform } from '~/util/Platform';
import { AddLocationDialog } from './AddLocationDialog';

export const AddLocationButton = (props: ButtonProps) => {
	const platform = usePlatform();
	const submitPlausibleEvent = usePlausibleEvent({ platformType: platform.platform });
	const createLocation = useLibraryMutation('locations.create', {
		onSuccess: () => {
			submitPlausibleEvent({
				event: {
					type: 'locationCreate'
				}
			});
		}
	});
	const indexerRulesList = useLibraryQuery(['locations.indexer_rules.list']);

	return (
		<>
			<Button
				{...props}
				onClick={async () => {
					let path = '';
					if (platform.openDirectoryPickerDialog) {
						const _path = await platform.openDirectoryPickerDialog();
						if (!_path) return;
						if (typeof _path !== 'string') {
							// TODO: Should support for adding multiple locations simultaneously be added?
							showAlertDialog({
								title: 'Error',
								value: "Can't add multiple locations"
							});
							return;
						}
						path = _path;
					}

					await dialogManager.create((dp) => <AddLocationDialog path={path} {...dp} />);
				}}
			>
				Add Location
			</Button>
		</>
	);
};
