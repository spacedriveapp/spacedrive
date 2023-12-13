import { useBottomTabBarHeight } from '@react-navigation/bottom-tabs';
import { Text, View } from 'react-native';
import { Icon } from '~/components/icons/Icon';
import { tw, twStyle } from '~/lib/tailwind';
import { NetworkStackScreenProps } from '~/navigation/tabs/NetworkStack';

export default function NetworkScreen({ navigation }: NetworkStackScreenProps<'Network'>) {
	const height = useBottomTabBarHeight();

	return (
		<View style={twStyle('flex-1 items-center justify-center', { marginBottom: height })}>
			<Icon name="Globe" size={128} />
			<Text style={tw`mt-4 text-lg font-bold text-white`}>Your Local Network</Text>
			<Text style={tw`mt-1 max-w-sm text-center text-sm text-ink-dull`}>
				Other Spacedrive nodes on your LAN will appear here, along with your default OS
				network mounts.
			</Text>
		</View>
	);
}
