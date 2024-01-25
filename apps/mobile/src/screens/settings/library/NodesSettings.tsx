import React from 'react';
import { Text, View } from 'react-native';
import { isEnabled, useBridgeMutation, useDiscoveredPeers } from '@sd/client';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const NodesSettingsScreen = ({ navigation }: SettingsStackScreenProps<'NodesSettings'>) => {
	const onlineNodes = useDiscoveredPeers();
	const p2pPair = useBridgeMutation('p2p.pair', {
		onSuccess(data) {
			console.log(data);
		}
	});

	return (
		<View>
			<Text style={tw`text-ink`}>Pairing</Text>

			{[...onlineNodes.entries()].map(([id, node]) => (
				<View key={id} style={tw`flex`}>
					<Text style={tw`text-ink`}>{node.name}</Text>

					<Button
						onPress={() => {
							if (!isEnabled('p2pPairing')) {
								alert('P2P Pairing is not enabled!');
							}

							// TODO: This is not great
							p2pPair.mutateAsync(id).then((id) => {
								// TODO: Show UI lmao
								// startPairing(id, {
								// 	name: node.name,
								// 	os: node.operating_system
								// });
							});
						}}
					>
						<Text>Pair</Text>
					</Button>
				</View>
			))}
		</View>
	);
};

export default NodesSettingsScreen;
