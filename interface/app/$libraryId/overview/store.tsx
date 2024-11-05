import { proxy, subscribe, useSnapshot } from 'valtio';
import { subscribeKey } from 'valtio/utils';
import { valtioPersist } from '@sd/client';

import FavoriteItems from './cards/FavoriteItems';
import FileKindStats from './cards/FileKindStats';
import LibraryStatistics from './cards/LibraryStats';
import RecentFiles from './cards/RecentItems';
import RecentLocations from './cards/RecentLocations';

export type CardSize = 'small' | 'medium' | 'large';

export interface CardConfig {
	id: string;
	enabled: boolean;
	size: CardSize;
	component: JSX.Element;
	title: string;
}

interface OverviewStore {
	cards: CardConfig[];
}

export const overviewStore = proxy<OverviewStore>({
	cards: [
		{
			id: 'library-stats',
			enabled: true,
			size: 'large',
			component: <LibraryStatistics />,
			title: 'Library Statistics'
		},
		{
			id: 'favorites',
			enabled: true,
			size: 'small',
			component: <FavoriteItems />,
			title: 'Favorites'
		},
		{
			id: 'file-kind-stats',
			enabled: true,
			size: 'small',
			component: <FileKindStats />,
			title: 'File Kinds'
		},
		{
			id: 'recent-files',
			enabled: true,
			size: 'small',
			component: <RecentFiles />,
			title: 'Recent Files'
		},
		{
			id: 'recent-locations',
			enabled: true,
			size: 'small',
			component: <RecentLocations />,
			title: 'Recent Locations'
		}
	]
});

// Persist store
export const layoutStore = valtioPersist('sd-overview', overviewStore);
