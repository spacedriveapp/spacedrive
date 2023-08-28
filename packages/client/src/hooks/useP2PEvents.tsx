import {
	MutableRefObject,
	PropsWithChildren,
	createContext,
	useContext,
	useEffect,
	useRef,
	useState
} from 'react';
import { P2PEvent, PairingStatus, PeerMetadata } from '../core';
import { useBridgeSubscription } from '../rspc';

type Context = {
	discoveredPeers: Map<string, PeerMetadata>;
	connectedPeers: Map<string, undefined>;
	pairingStatus: Map<number, PairingStatus>;
	events: MutableRefObject<EventTarget>;
};

const Context = createContext<Context>(null as any);

export function P2PContextProvider({ children }: PropsWithChildren) {
	const events = useRef(new EventTarget());
	const [[discoveredPeers], setDiscoveredPeer] = useState([new Map<string, PeerMetadata>()]);
	const [[connectedPeers], setConnectedPeers] = useState([new Map<string, undefined>()]);
	const [[pairingStatus], setPairingStatus] = useState([new Map<number, PairingStatus>()]);

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			events.current.dispatchEvent(new CustomEvent('p2p-event', { detail: data }));

			if (data.type === 'DiscoveredPeer') {
				discoveredPeers.set(data.peer_id, data.metadata);
				setDiscoveredPeer([discoveredPeers]);
			} else if (data.type === 'ExpiredPeer') {
				discoveredPeers.delete(data.peer_id);
				setDiscoveredPeer([discoveredPeers]);
			} else if (data.type === 'ConnectedPeer') {
				connectedPeers.set(data.peer_id, undefined);
				setConnectedPeers([connectedPeers]);
			} else if (data.type === 'DisconnectedPeer') {
				connectedPeers.delete(data.peer_id);
				setConnectedPeers([connectedPeers]);
			} else if (data.type === 'PairingProgress') {
				setPairingStatus([pairingStatus.set(data.id, data.status)]);
			}
		}
	});

	return (
		<Context.Provider
			value={{
				discoveredPeers,
				connectedPeers,
				pairingStatus,
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

export function usePairingStatus(pairing_id: number) {
	return useContext(Context).pairingStatus.get(pairing_id);
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
