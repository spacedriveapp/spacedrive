import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface ViewSettings {
	gridSize: number;
	gapSize: number;
	showFileSize: boolean;
	columnWidth: number;
}

interface LayoutItemViewPreferences {
	viewMode: 'grid' | 'list' | 'media' | 'column' | 'size' | 'knowledge';
	viewSettings: ViewSettings;
}

interface ViewPreferencesStore {
	preferences: Record<string, LayoutItemViewPreferences>;
	getPreferences: (layoutItemId: string) => LayoutItemViewPreferences | undefined;
	setPreferences: (layoutItemId: string, prefs: Partial<LayoutItemViewPreferences>) => void;
}

const DEFAULT_VIEW_SETTINGS: ViewSettings = {
	gridSize: 120,
	gapSize: 16,
	showFileSize: true,
	columnWidth: 256,
};

const DEFAULT_PREFERENCES: LayoutItemViewPreferences = {
	viewMode: 'grid',
	viewSettings: DEFAULT_VIEW_SETTINGS,
};

export const useViewPreferencesStore = create<ViewPreferencesStore>()(
	persist(
		(set, get) => ({
			preferences: {},
			getPreferences: (layoutItemId) => {
				return get().preferences[layoutItemId];
			},
			setPreferences: (layoutItemId, prefs) =>
				set((state) => ({
					preferences: {
						...state.preferences,
						[layoutItemId]: {
							...DEFAULT_PREFERENCES,
							...state.preferences[layoutItemId],
							...prefs,
							viewSettings: {
								...DEFAULT_VIEW_SETTINGS,
								...state.preferences[layoutItemId]?.viewSettings,
								...(prefs.viewSettings || {}),
							},
						},
					},
				})),
		}),
		{
			name: 'spacedrive-view-preferences',
		}
	)
);
