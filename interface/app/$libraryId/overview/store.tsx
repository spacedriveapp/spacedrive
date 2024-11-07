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
		id: 'space-wizard',
		enabled: true,
		size: 'medium',
		title: 'Organizer'
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
		id: 'sync-cta',
		enabled: true,
		size: 'small',
		title: 'Enable Sync'
	},
	{
		id: 'file-kind-stats',
		enabled: true,
		size: 'small',
		title: 'File Kinds'
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
	},
	{
		id: 'storage-meters',
		enabled: true,
		size: 'medium',
		title: 'Storage Meters'
	}
];

export const state = proxy<OverviewStore>({
	cards: defaultCards
});

// Persist store
export const overviewStore = valtioPersist('sd-overview-layout', state, {
	saveFn: (data) => data,

	// Restore the cards with the default values while allowing new cards to be added
	restoreFn: (stored) => ({
		...state,
		...stored,
		cards: defaultCards.map((defaultCard) => ({
			...defaultCard,
			...stored.cards.find((card: CardConfig) => card.id === defaultCard.id)
		}))
	})
});
