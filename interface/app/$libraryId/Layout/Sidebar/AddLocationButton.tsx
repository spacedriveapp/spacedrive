import { useLibraryMutation } from '@sd/client';
import { dialogManager } from '@sd/ui';
import { usePlatform } from '~/util/Platform';
import AddLocationDialog from '../../settings/library/locations/AddDialog';

export default () => {
	const platform = usePlatform();

	const createLocation = useLibraryMutation('locations.create');

	return (
		<button
			onClick={() => {
				if (platform.platform === 'web') {
					dialogManager.create((dp) => <AddLocationDialog {...dp} />);
				} else {
					if (!platform.openDirectoryPickerDialog) {
						alert('Opening a dialogue is not supported on this platform!');
						return;
					}
					platform.openDirectoryPickerDialog().then((result) => {
						// TODO: Pass indexer rules ids to create location
						if (result)
							createLocation.mutate({
								path: result as string,
								indexer_rules_ids: []
							});
					});
				}
			}}
			className="
				border-sidebar-line hover:border-sidebar-selected cursor-normal text-ink-faint mt-1 w-full rounded
				border border-dashed px-2 py-1 text-center
				text-xs font-medium transition
			"
		>
			Add Location
		</button>
	);
};
