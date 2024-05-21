import React from 'react';
import { Text, View } from 'react-native';
import { useDiscoveredPeers } from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const NodesSettingsScreen = ({ navigation }: SettingsStackScreenProps<'NodesSettings'>) => {
	const onlineNodes = useDiscoveredPeers();

	return (
		<ScreenContainer scrollview={false} style={tw`gap-0 px-6`}>
			<Text style={tw`text-ink`}>Pairing</Text>

			{[...onlineNodes.entries()].map(([id, node]) => (
				<View key={id} style={tw`flex`}>
					<Text style={tw`text-ink`}>{node.metadata.name}</Text>
				</View>
			))}
		</ScreenContainer>
	);
};

export default NodesSettingsScreen;
