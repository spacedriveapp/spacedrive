import { initRspc, wsBatchLink, type AlphaClient } from '@oscartbeaumont-sd/rspc-client/v2';
import { useEffect, useMemo, useRef, useState } from 'react';
import {
	EphemeralPathOrder,
	ExplorerItem,
	getExplorerItemData,
	useDiscoveredPeers,
	useLibraryContext,
	useNormalisedCache,
	useUnsafeStreamedQuery,
	type Procedures
} from '@sd/client';
import { Button } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale, useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import Explorer from '../Explorer';
import { ExplorerContextProvider } from '../Explorer/Context';
import {
	createDefaultExplorerSettings,
	explorerStore,
	nonIndexedPathOrderingSchema
} from '../Explorer/store';
import { useExplorer, useExplorerSettings } from '../Explorer/useExplorer';
import { EmptyNotice } from '../Explorer/View/EmptyNotice';

export const Component = () => {
	// TODO: Handle if P2P is disabled
	const [activePeer, setActivePeer] = useState<string | null>(null);

	return (
		<div className="p-4">
			{activePeer ? (
				<P2PInfo peer={activePeer} resetActivePeer={() => setActivePeer(null)} />
			) : (
				<PeerSelector setActivePeer={setActivePeer} />
			)}
		</div>
	);
};

function PeerSelector({ setActivePeer }: { setActivePeer: (peer: string) => void }) {
	const peers = useDiscoveredPeers();

	return (
		<>
			<h1>Nodes:</h1>
			{peers.size === 0 ? (
				<p>No peers found...</p>
			) : (
				<ul>
					{[...peers.entries()].map(([id, _node]) => (
						<li key={id}>
							{id}
							<Button onClick={() => setActivePeer(id)}>Connect</Button>
						</li>
					))}
				</ul>
			)}
		</>
	);
}

function P2PInfo({ peer, resetActivePeer }: { peer: string; resetActivePeer: () => void }) {
	const platform = usePlatform();
	const ref = useRef<AlphaClient<Procedures>>();
	const [result, setResult] = useState('');
	useEffect(() => {
		// TODO: Cleanup when URL changed
		const endpoint = platform.getRemoteRspcEndpoint(peer);
		ref.current = initRspc<Procedures>({
			links: [
				wsBatchLink({
					url: endpoint.url
				})
			]
		});
	}, [peer]);

	useEffect(() => {
		if (!ref.current) return;
		ref.current.query(['nodeState']).then((data) => setResult(JSON.stringify(data, null, 2)));
	}, [ref, result]);

	const path = '/'; // TODO: Account for windows or linux on the remote node

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<EphemeralPathOrder>({
					order: {
						field: 'name',
						value: 'Asc'
					}
				}),
			[]
		),
		orderingKeys: nonIndexedPathOrderingSchema
	});

	const libraryCtx = useLibraryContext();
	const settingsSnapshot = explorerSettings.useSettingsSnapshot();
	const cache = useNormalisedCache();
	const os = useOperatingSystem();
	const { t } = useLocale();

	const query = useUnsafeStreamedQuery(
		[
			'search.ephemeralPaths',
			{
				library_id: libraryCtx.library.uuid,
				arg: {
					path: path ?? (os === 'windows' ? 'C:\\' : '/'),
					withHiddenFiles: settingsSnapshot.showHiddenFiles,
					order: settingsSnapshot.order
				}
			}
		],
		{
			// enabled: path != null,
			suspense: true,
			onSuccess: () => explorerStore.resetNewThumbnails(),
			onBatch: (item) => {
				cache.withNodes(item.nodes);
			}
		}
	);

	const entries = useMemo(() => {
		return cache.withCache(
			query.data?.flatMap((item) => item.entries) ||
				query.streaming.flatMap((item) => item.entries)
		);
	}, [cache, query.streaming, query.data]);

	const items = useMemo(() => {
		if (!entries) return [];

		const ret: ExplorerItem[] = [];

		for (const item of entries) {
			if (settingsSnapshot.layoutMode !== 'media') ret.push(item);
			else {
				const { kind } = getExplorerItemData(item);

				if (kind === 'Video' || kind === 'Image') ret.push(item);
			}
		}

		return ret;
	}, [entries, settingsSnapshot.layoutMode]);

	const explorer = useExplorer({
		items,
		parent: path != null ? { type: 'Ephemeral', path } : undefined,
		settings: explorerSettings,
		layouts: { media: false }
	});

	// useKeyDeleteFile(explorer.selectedItems, null);

	return (
		<div className="flex flex-col">
			<Button onClick={() => resetActivePeer()}>Disconnect</Button>
			<h1>Connected with: {peer}</h1>

			<Button
				onClick={() => {
					ref.current
						?.query(['nodeState'])
						.then((data) => setResult(JSON.stringify(data, null, 2)));
				}}
			>
				Refetch
			</Button>
			<pre>{result}</pre>
			<ExplorerContextProvider explorer={explorer}>
				{/* <TopBarPortal
					left={
						<Tooltip label={t('add_location_tooltip')} className="w-max min-w-0 shrink">
							<AddLocationButton path={path} />
						</Tooltip>
					}
					right={<DefaultTopBarOptions />}
				/> */}
				<Explorer
					emptyNotice={
						<EmptyNotice
							loading={query.isFetching}
							icon={<Icon name="FolderNoSpace" size={128} />}
							message={t('no_files_found_here')}
						/>
					}
				/>
			</ExplorerContextProvider>
		</div>
	);
}
