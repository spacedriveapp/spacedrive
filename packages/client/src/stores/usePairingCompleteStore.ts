import create from 'zustand';

type PeerMetadata = void; // TODO

interface PairingCompleteStore {
	pairingRequestCallbacks: Map<string, (peer_metadata: PeerMetadata) => void>;
}

export const usePairingCompleteStore = create<PairingCompleteStore>((set) => ({
	pairingRequestCallbacks: new Map<string, (peer_metadata: PeerMetadata) => void>()
}));
