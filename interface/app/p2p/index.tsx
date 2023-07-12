import { useOnFeatureFlagsChange, useP2PEvents, withFeatureFlag } from '@sd/client';
import { SpacedropUI } from './Spacedrop';
import { startPairing } from './pairing';

export const SpacedropUI2 = withFeatureFlag('spacedrop', SpacedropUI);

// Entrypoint of P2P UI
export function P2P() {
	useP2PEvents((data) => {
		if (data.type === 'PairingRequest') {
			startPairing(data.id, {
				name: data.name,
				os: data.os
			});
		}
	});

	return (
		<>
			<SpacedropUI2 />
		</>
	);
}
