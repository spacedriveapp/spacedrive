import { useMemo } from 'react';
import { useDebugState, useDiscoveredPeers, useFeatureFlag, useFeatureFlags } from '@sd/client';
import { Icon } from '~/components';
import { useRouteTitle } from '~/hooks/useRouteTitle';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { createDefaultExplorerSettings, nonIndexedPathOrderingSchema } from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { TopBarPortal } from './TopBar/Portal';

export const Component = () => {
	const title = useRouteTitle('Network');

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
			type: 'SpacedropPeer',
			has_local_thumbnail: false,
			thumbnail_key: null,
			item: {
				...peer,
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
						<h1 className="mt-4 text-lg font-bold">Your Local Network</h1>
						<p className="mt-1 max-w-sm text-center text-sm text-ink-dull">
							Other Spacedrive nodes on your LAN will appear here, along with your
							default OS network mounts.
						</p>
						<Debug />
					</div>
				}
			/>
		</ExplorerContextProvider>
	);
};

function Debug() {
	const debugState = useDebugState();
	const featureFlags = useFeatureFlags();
	const demo = useFeatureFlag('solidJsDemo');

	return (
		<>
			<p className="text-red">{debugState.enabled ? 'Enabled' : 'Disabled'}</p>
			<button
				onClick={() => {
					debugState.enabled = !debugState.enabled;
				}}
			>
				Toggle
			</button>
			<p className="text-red">{JSON.stringify(featureFlags)}</p>
			<p className="text-red">{demo ? 'Enabled' : 'Disabled'}</p>
		</>
	);
}
