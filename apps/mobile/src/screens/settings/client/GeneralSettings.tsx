import { Text, View } from 'react-native';
import { useBridgeQuery, useDebugState } from '@sd/client';
import { Input } from '~/components/form/Input';
import Card from '~/components/layout/Card';
import { Divider } from '~/components/primitive/Divider';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const GeneralSettingsScreen = ({ navigation }: SettingsStackScreenProps<'GeneralSettings'>) => {
	const { data: node } = useBridgeQuery(['nodeState']);

	const debugState = useDebugState();

	if (!node) return null;

	return (
		<View style={tw`flex-1 p-4`}>
			<Card style={tw`bg-app-box`}>
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
				<Divider style={tw`mb-4 mt-2`} />
				{/* Node Name and Port */}
				<SettingsTitle>Node Name</SettingsTitle>
				<Input value={node.name} />
				<SettingsTitle style={tw`mt-3`}>Node Port</SettingsTitle>
				<Input value={node.p2p_port?.toString() ?? '5795'} keyboardType="numeric" />
			</Card>
			{debugState.enabled && (
				<Card style={tw`mt-4`}>
					{/* Card Header */}
					<Text style={tw`font-semibold text-ink`}>Debug</Text>
					{/* Divider */}
					<Divider style={tw`mb-4 mt-2`} />
					<SettingsTitle>Data Folder</SettingsTitle>
					{/* Useful for simulator, not so for real devices. */}
					<Input value={node.data_path} />
				</Card>
			)}
		</View>
	);
};

export default GeneralSettingsScreen;
