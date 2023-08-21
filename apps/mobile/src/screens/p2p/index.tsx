import { useFeatureFlag, useP2PEvents } from '@sd/client';

export function P2P() {
	// const pairingResponse = useBridgeMutation('p2p.pairingResponse');
	// const activeLibrary = useLibraryContext();

	const pairingEnabled = useFeatureFlag('p2pPairing');
	useP2PEvents((data) => {
		if (data.type === 'PairingRequest' && pairingEnabled) {
			console.log('Pairing incoming from', data.name);

			// TODO: open pairing screen and guide user through the process. For now we auto-accept
			// pairingResponse.mutate([
			// 	data.id,
			// 	{ decision: 'accept', libraryId: activeLibrary.library.uuid }
			// ]);
		}

		// TODO: For now until UI is implemented
		if (data.type === 'PairingProgress') {
			console.log('Pairing progress', data);
		}
	});

	return null;
}
