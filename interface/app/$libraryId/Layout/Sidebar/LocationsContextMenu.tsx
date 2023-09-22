import { Pencil, Plus, Trash } from '@phosphor-icons/react';
import { useNavigate } from 'react-router';
import { ContextMenu as CM, dialogManager, toast } from '@sd/ui';
import { AddLocationDialog } from '~/app/$libraryId/settings/library/locations/AddLocationDialog';
import DeleteDialog from '~/app/$libraryId/settings/library/locations/DeleteDialog';
import { openDirectoryPickerDialog } from '~/app/$libraryId/settings/library/locations/openDirectoryPickerDialog';
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
				onClick={async () => {
					try {
						const path = await openDirectoryPickerDialog(platform);
						if (path !== '') {
							dialogManager.create((dp) => (
								<AddLocationDialog path={path ?? ''} {...dp} />
							));
						}
					} catch (error) {
						toast.error(`${error}`);
					}
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
