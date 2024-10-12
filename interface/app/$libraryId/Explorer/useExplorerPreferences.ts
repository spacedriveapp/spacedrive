import { useMemo } from 'react';

import {
	ExplorerSettings,
	LibraryPreferences,
	Ordering,
	useExplorerLayoutStore,
	useLibraryMutation,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';

import { createDefaultExplorerSettings } from './store';

// preferences are settings persisted to the db and synced
export function useExplorerPreferences<TData, TOrder extends Ordering>({
	data,
	createDefaultSettings,
	getSettings,
	writeSettings
}: {
	data: TData;
	createDefaultSettings(): ReturnType<typeof createDefaultExplorerSettings<TOrder>>;
	getSettings: (prefs: LibraryPreferences) => ExplorerSettings<TOrder> | undefined;
	writeSettings: (settings: ExplorerSettings<TOrder>) => LibraryPreferences;
}) {
	const rspc = useRspcLibraryContext();
	const explorerLayout = useExplorerLayoutStore();

	const preferences = useLibraryQuery(['preferences.get']);
	const updatePreferences = useLibraryMutation('preferences.update');

	const settings = useMemo(() => {
		const defaults = createDefaultSettings();

		if (!location || !preferences.data) return defaults;

		const settings = getSettings(preferences.data);

		// Overwrite the default layout with the user's preference
		Object.assign(defaults, { layoutMode: explorerLayout.defaultView });

		if (!settings) return defaults;

		for (const [key, value] of Object.entries(settings)) {
			if (value !== null) Object.assign(defaults, { [key]: value });
		}

		return defaults;
	}, [preferences.data, getSettings, createDefaultSettings, explorerLayout.defaultView]);

	const onSettingsChanged = async (settings: ExplorerSettings<TOrder>) => {
		if (preferences.isLoading) return;

		try {
			await updatePreferences.mutateAsync(writeSettings(settings));
			rspc.queryClient.invalidateQueries({ queryKey: ['preferences.get'] });
		} catch (e) {
			alert('An error has occurred while updating your preferences.');
		}
	};

	return Object.assign(preferences, {
		explorerSettingsProps: {
			settings,
			onSettingsChanged,
			data
		}
	});
}
