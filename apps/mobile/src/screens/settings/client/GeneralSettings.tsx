import React from 'react';
import { Text, View } from 'react-native';
import { useBridgeQuery } from '@sd/client';
import Card from '~/components/layout/Card';
import Divider from '~/components/primitive/Divider';
import { Input } from '~/components/primitive/Input';
import tw from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const GeneralSettingsScreen = ({ navigation }: SettingsStackScreenProps<'GeneralSettings'>) => {
	const { data: node } = useBridgeQuery(['nodeState']);

	if (!node) return null;

	return (
		<View style={tw`flex-1 p-4`}>
			<Card>
				{/* Card Header */}
				<View style={tw`flex flex-row justify-between`}>
					<Text style={tw`font-semibold text-ink`}>Connected Node</Text>
					<View style={tw`flex flex-row`}>
						{/* Peers */}
						<View style={tw`mr-2 self-start rounded bg-app-highlight px-1.5 py-[2px]`}>
							<Text style={tw`text-xs font-semibold text-ink`}>0 Peers</Text>
						</View>
						{/* Status */}
						<View style={tw`rounded bg-accent px-1.5 py-[2px]`}>
							<Text style={tw`text-xs font-semibold text-ink`}>Running</Text>
						</View>
					</View>
				</View>
				{/* Divider */}
				<Divider style={tw`mt-2 mb-4`} />
				{/* Node Name and Port */}
				<Text style={tw`mb-1 ml-1 text-xs font-medium text-ink-dull`}>Node Name</Text>
				<Input value={node.name} />
				<Text style={tw`mt-2 mb-1 ml-1 text-xs font-medium text-ink-dull`}>Node Port</Text>
				<Input value={node.p2p_port?.toString() ?? '5795'} keyboardType="numeric" />
			</Card>
		</View>
	);
};

export default GeneralSettingsScreen;
