import { Text, View } from 'react-native';
import { useBridgeQuery, useDebugState } from '@sd/client';
import Card from '~/components/layout/Card';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Divider } from '~/components/primitive/Divider';
import { Input } from '~/components/primitive/Input';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const GeneralSettingsScreen = ({ navigation }: SettingsStackScreenProps<'GeneralSettings'>) => {
	const { data: node } = useBridgeQuery(['nodeState']);

	const debugState = useDebugState();

	if (!node) return null;

	return (
		<ScreenContainer style={tw`justify-start gap-0 px-6`} scrollview={false}>
			<Card>
				{/* Card Header */}
				<View style={tw`flex flex-row justify-between`}>
					<Text style={tw`font-semibold text-ink`}>Connected Node</Text>
					<View style={tw`flex flex-row`}>
						{/* Peers */}
						<View
							style={tw`mr-2 self-start rounded border border-app-lightborder bg-app-highlight px-1.5 py-[2px]`}
						>
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
				<SettingsTitle style={tw`mb-1`}>Node Name</SettingsTitle>
				<Input value={node.name} />
				{/* // TODO: Bring this back */}
				{/* <SettingsTitle style={tw`mt-3 mb-1`}>Node Port</SettingsTitle> */}
				{/* <Input value={node.p2p_port?.toString() ?? '5795'} keyboardType="numeric" /> */}
			</Card>
			{debugState.enabled && (
				<Card style={tw`mt-4`}>
					{/* Card Header */}
					<Text style={tw`font-semibold text-ink`}>Debug</Text>
					{/* Divider */}
					<Divider style={tw`mb-4 mt-2`} />
					<SettingsTitle style={tw`mb-1`}>Data Folder</SettingsTitle>
					{/* Useful for simulator, not so for real devices. */}
					<Input value={node.data_path} />
				</Card>
			)}
		</ScreenContainer>
	);
};

export default GeneralSettingsScreen;
