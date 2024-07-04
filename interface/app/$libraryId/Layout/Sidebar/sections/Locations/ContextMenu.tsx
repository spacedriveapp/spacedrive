import { Pencil, Plus, Trash } from '@phosphor-icons/react';
import { PropsWithChildren } from 'react';
import { useNavigate } from 'react-router';
import { useLibraryContext } from '@sd/client';
import { ContextMenu as CM, dialogManager, toast } from '@sd/ui';
import { AddLocationDialog } from '~/app/$libraryId/settings/library/locations/AddLocationDialog';
import DeleteDialog from '~/app/$libraryId/settings/library/locations/DeleteDialog';
import { openDirectoryPickerDialog } from '~/app/$libraryId/settings/library/locations/openDirectoryPickerDialog';
import { useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { useSidebarContext } from '../../SidebarLayout/Context';

export const ContextMenu = ({
	children,
	locationId
}: PropsWithChildren<{ locationId: number }>) => {
	const navigate = useNavigate();
	const platform = usePlatform();
	const libraryId = useLibraryContext().library.uuid;

	const sidebar = useSidebarContext();

	const { t } = useLocale();

	return (
		<CM.Root
			trigger={children}
			onOpenChange={(open) => sidebar.onLockedChange(open)}
			className="z-[100]"
		>
			<CM.Item
				onClick={async () => {
					try {
						const path = await openDirectoryPickerDialog(platform);
						if (path !== '') {
							dialogManager.create((dp) => (
								<AddLocationDialog
									path={path ?? ''}
									libraryId={libraryId}
									{...dp}
								/>
							));
						}
					} catch (error) {
						toast.error(t('error_message', { error }));
					}
				}}
				icon={Plus}
				label={t('new_location')}
			/>
			<CM.Item
				onClick={() => {
					navigate(`settings/library/locations/${locationId}`);
				}}
				icon={Pencil}
				label={t('edit')}
			/>
			<CM.Separator />
			<CM.Item
				icon={Trash}
				label={t('delete')}
				variant="danger"
				onClick={(e: { stopPropagation: () => void }) => {
					e.stopPropagation();
					dialogManager.create((dp) => (
						<DeleteDialog
							{...dp}
							onSuccess={() => {
								toast.success(t('location_deleted_successfully'));
								navigate('settings/library/locations');
							}}
							locationId={locationId}
						/>
					));
				}}
			/>
		</CM.Root>
	);
};
