import {
	createContext,
	MutableRefObject,
	PropsWithChildren,
	useContext,
	useEffect,
	useRef,
	useState
} from 'react';

import { ConnectionMethod, DiscoveryMethod, P2PEvent, PeerMetadata } from '../core';
import { useBridgeSubscription } from '../rspc';

type Peer = {
	connection: ConnectionMethod;
	discovery: DiscoveryMethod;
	metadata: PeerMetadata;
};

type Context = {
	peers: Map<string, Peer>;
	spacedropProgresses: Map<string, number>;
	events: MutableRefObject<EventTarget>;
};

const Context = createContext<Context>(null as any);

export function P2PContextProvider({ children }: PropsWithChildren) {
	const events = useRef(new EventTarget());
	const [[peers], setPeers] = useState([new Map<string, Peer>()]);
	const [[spacedropProgresses], setSpacedropProgresses] = useState([new Map<string, number>()]);

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			events.current.dispatchEvent(new CustomEvent('p2p-event', { detail: data }));

			if (data.type === 'PeerChange') {
				peers.set(data.identity, {
					connection: data.connection,
					discovery: data.discovery,
					metadata: data.metadata
				});
				setPeers([peers]);
			} else if (data.type === 'PeerDelete') {
				peers.delete(data.identity);
				setPeers([peers]);
			} else if (data.type === 'SpacedropProgress') {
				spacedropProgresses.set(data.id, data.percent);
				setSpacedropProgresses([spacedropProgresses]);
			}
		}
	});

	return (
		<Context.Provider
			value={{
				peers,
				spacedropProgresses,
				events
			}}
		>
			{children}
		</Context.Provider>
	);
}

export function useP2PContextRaw() {
	return useContext(Context);
}

export function usePeers() {
	return useContext(Context).peers;
}

export function useDiscoveredPeers() {
	return new Map([...usePeers()].filter(([, peer]) => peer.connection === 'Disconnected'));
}

export function useConnectedPeers() {
	return new Map([...usePeers()].filter(([, peer]) => peer.connection !== 'Disconnected'));
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
