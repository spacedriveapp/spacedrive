import { proxy, subscribe, useSnapshot } from 'valtio';
import { subscribeKey } from 'valtio/utils';
import { valtioPersist } from '@sd/client';

export type CardSize = 'small' | 'medium' | 'large';

export interface CardConfig {
	id: string;
	enabled: boolean;
	size: CardSize;
	title: string;
}

interface OverviewStore {
	cards: CardConfig[];
}

export const defaultCards: CardConfig[] = [
	{
		id: 'library-stats',
		enabled: true,
		size: 'medium',
		title: 'Library Statistics'
	},
	{
		id: 'file-kind-stats',
		enabled: true,
		size: 'medium',
		title: 'File Kinds'
	},
	{
		id: 'favorites',
		enabled: true,
		size: 'small',
		title: 'Favorites'
	},
	{
		id: 'recent-locations',
		enabled: true,
		size: 'medium',
		title: 'Recent Locations'
	},
	{
		id: 'device-list',
		enabled: true,
		size: 'small',
		title: 'Devices'
	},

	{
		id: 'recent-files',
		enabled: true,
		size: 'medium',
		title: 'Recent Files'
	}
];

export const state = proxy<OverviewStore>({
	cards: defaultCards
});

// Persist store
export const overviewStore = valtioPersist('sd-overview-layout', state);
