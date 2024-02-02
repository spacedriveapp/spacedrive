import {
	createContext,
	MutableRefObject,
	PropsWithChildren,
	useContext,
	useEffect,
	useRef,
	useState
} from 'react';

import { P2PEvent, PeerMetadata } from '../core';
import { useBridgeSubscription } from '../rspc';

type Context = {
	discoveredPeers: Map<string, PeerMetadata>;
	connectedPeers: Map<string, undefined>;
	spacedropProgresses: Map<string, number>;
	events: MutableRefObject<EventTarget>;
};

const Context = createContext<Context>(null as any);

export function P2PContextProvider({ children }: PropsWithChildren) {
	const events = useRef(new EventTarget());
	const [[discoveredPeers], setDiscoveredPeer] = useState([new Map<string, PeerMetadata>()]);
	const [[connectedPeers], setConnectedPeers] = useState([new Map<string, undefined>()]);
	const [[spacedropProgresses], setSpacedropProgresses] = useState([new Map<string, number>()]);

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			events.current.dispatchEvent(new CustomEvent('p2p-event', { detail: data }));

			if (data.type === 'DiscoveredPeer') {
				discoveredPeers.set(data.identity, data.metadata);
				setDiscoveredPeer([discoveredPeers]);
			} else if (data.type === 'ExpiredPeer') {
				discoveredPeers.delete(data.identity);
				setDiscoveredPeer([discoveredPeers]);
			} else if (data.type === 'ConnectedPeer') {
				connectedPeers.set(data.identity, undefined);
				setConnectedPeers([connectedPeers]);
			} else if (data.type === 'DisconnectedPeer') {
				connectedPeers.delete(data.identity);
				setConnectedPeers([connectedPeers]);
			} else if (data.type === 'SpacedropProgress') {
				spacedropProgresses.set(data.id, data.percent);
				setSpacedropProgresses([spacedropProgresses]);
			}
		}
	});

	return (
		<Context.Provider
			value={{
				discoveredPeers,
				connectedPeers,
				spacedropProgresses,
				events
			}}
		>
			{children}
		</Context.Provider>
	);
}

export function useDiscoveredPeers() {
	return useContext(Context).discoveredPeers;
}

export function useConnectedPeers() {
	return useContext(Context).connectedPeers;
}

export function useSpacedropProgress(id: string) {
	return useContext(Context).spacedropProgresses.get(id);
}

export function useP2PEvents(fn: (event: P2PEvent) => void) {
	const ctx = useContext(Context);

	useEffect(() => {
		const handler = (e: Event) => {
			fn((e as any).detail);
		};

		ctx.events.current.addEventListener('p2p-event', handler);
		return () => ctx.events.current.removeEventListener('p2p-event', handler);
	});
}
