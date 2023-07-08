import {
	MutableRefObject,
	PropsWithChildren,
	createContext,
	useContext,
	useEffect,
	useRef,
	useState
} from 'react';
import { P2PEvent, PeerMetadata } from '../core';
import { useBridgeSubscription } from '../rspc';

type Context = {
	discoveredPeers: Map<string, PeerMetadata>;
	events: MutableRefObject<EventTarget>;
};

const Context = createContext<Context>(null as any);

export function P2PContextProvider({ children }: PropsWithChildren) {
	const events = useRef(new EventTarget());
	const [[discoveredPeers], setDiscoveredPeer] = useState([new Map<string, PeerMetadata>()]);

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			if (data.type === 'DiscoveredPeer') {
				setDiscoveredPeer([discoveredPeers.set(data.peer_id, data.metadata)]);
			}
			events.current.dispatchEvent(new CustomEvent<P2PEvent>('p2p-event', { detail: data }));
		}
	});

	return (
		<Context.Provider
			value={{
				discoveredPeers,
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
