import { Text, View } from 'react-native';
import { Icon } from '~/components/icons/Icon';
import { tw } from '~/lib/tailwind';
import { NetworkStackScreenProps } from '~/navigation/tabs/NetworkStack';

export default function NetworkScreen({ navigation }: NetworkStackScreenProps<'Network'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Icon name="Globe" size={128} />
			<Text style={tw`mt-4 text-lg font-bold text-white`}>Your Local Network</Text>
			<Text style={tw`mt-1 max-w-sm text-center text-sm text-ink-dull`}>
				Other Spacedrive nodes on your LAN will appear here, along with your default OS
				network mounts.
			</Text>
		</View>
	);
}
