import { PropsWithChildren, createContext, useContext, useState } from 'react';
import { PeerMetadata } from '../core';
import { useBridgeSubscription } from '../rspc';

const Context = createContext<Map<string, PeerMetadata>>(null as any);

export function P2PContextProvider({ children }: PropsWithChildren) {
	const [[discoveredPeers], setDiscoveredPeer] = useState([new Map<string, PeerMetadata>()]);

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			if (data.type === 'DiscoveredPeer') {
				setDiscoveredPeer([discoveredPeers.set(data.peer_id, data.metadata)]);
			}
		}
	});

	return <Context.Provider value={discoveredPeers}>{children}</Context.Provider>;
}

export function useDiscoveredPeers() {
	return useContext(Context);
}
