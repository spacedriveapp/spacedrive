import { Pencil, Plus, Trash } from 'phosphor-react';
import { useNavigate } from 'react-router';
import { ContextMenu as CM, dialogManager } from '@sd/ui';
import {
	AddLocationDialog,
	openDirectoryPickerDialog
} from '~/app/$libraryId/settings/library/locations/AddLocationDialog';
import DeleteDialog from '~/app/$libraryId/settings/library/locations/DeleteDialog';
import { showAlertDialog } from '~/components';
import { usePlatform } from '~/util/Platform';

interface Props {
	children: React.ReactNode;
	locationId: number;
}

export default ({ children, locationId }: Props) => {
	const navigate = useNavigate();
	const platform = usePlatform();
	return (
		<CM.Root trigger={children}>
			<CM.Item
				onClick={() => {
					openDirectoryPickerDialog(platform)
						.then((path) => {
							if (path !== '')
								dialogManager.create((dp) => (
									<AddLocationDialog path={path ?? ''} {...dp} />
								));
						})
						.catch((error) =>
							showAlertDialog({
								title: 'Error',
								value: String(error)
							})
						);
				}}
				icon={Plus}
				label="New location"
			/>
			<CM.Item
				onClick={() => {
					navigate(`settings/library/locations/${locationId}`);
				}}
				icon={Pencil}
				label="Edit"
			/>
			<CM.Separator />
			<CM.Item
				icon={Trash}
				label="Delete"
				variant="danger"
				onClick={(e: { stopPropagation: () => void }) => {
					e.stopPropagation();
					dialogManager.create((dp) => (
						<DeleteDialog
							{...dp}
							onSuccess={() => navigate('settings/library/locations')}
							locationId={locationId}
						/>
					));
				}}
			/>
		</CM.Root>
	);
};
