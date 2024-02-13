import React from 'react';
import { Text, View } from 'react-native';
import { isEnabled, useBridgeMutation, useDiscoveredPeers } from '@sd/client';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const NodesSettingsScreen = ({ navigation }: SettingsStackScreenProps<'NodesSettings'>) => {
	const onlineNodes = useDiscoveredPeers();

	return (
		<View>
			<Text style={tw`text-ink`}>Pairing</Text>

			{[...onlineNodes.entries()].map(([id, node]) => (
				<View key={id} style={tw`flex`}>
					<Text style={tw`text-ink`}>{node.name}</Text>
				</View>
			))}
		</View>
	);
};

export default NodesSettingsScreen;
