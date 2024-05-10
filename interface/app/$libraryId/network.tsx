import { useMemo } from 'react';
import { nonIndexedPathOrderingSchema, useDiscoveredPeers } from '@sd/client';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { useRouteTitle } from '~/hooks/useRouteTitle';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { createDefaultExplorerSettings } from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { TopBarPortal } from './TopBar/Portal';

export const Component = () => {
	const title = useRouteTitle('Network');

	const { t } = useLocale();

	const discoveredPeers = useDiscoveredPeers();
	const peers = useMemo(() => Array.from(discoveredPeers.values()), [discoveredPeers]);

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings({
					order: {
						field: 'name',
						value: 'Asc'
					}
				}),
			[]
		),
		orderingKeys: nonIndexedPathOrderingSchema
	});

	const explorer = useExplorer({
		items: peers.map((peer) => ({
			type: 'SpacedropPeer' as const,
			has_local_thumbnail: false,
			thumbnail: null,
			item: {
				...peer.metadata,
				pub_id: []
			}
		})),
		settings: explorerSettings,
		layouts: { media: false }
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<Icon name="Globe" size={22} />
						<span className="truncate text-sm font-medium">{title}</span>
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>
			<Explorer
				emptyNotice={
					<div className="flex h-full flex-col items-center justify-center text-white">
						<Icon name="Globe" size={128} />
						<h1 className="mt-4 text-lg font-bold">{t('your_local_network')}</h1>
						<p className="mt-1 max-w-sm text-center text-sm text-ink-dull">
							{t('network_page_description')}
						</p>
					</div>
				}
			/>
		</ExplorerContextProvider>
	);
};
