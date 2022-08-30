import { Text, TouchableOpacity, View } from 'react-native';

import tw from '../lib/tailwind';
import type { RootStackScreenProps } from '../navigation';

export default function NotFoundScreen({ navigation }: RootStackScreenProps<'NotFound'>) {
	return (
		<View style={tw`flex-1 items-center justify-center p-5`}>
			<Text style={tw`font-bold text-xl`}>This screen doesn&apos;t exist.</Text>
			<TouchableOpacity onPress={() => navigation.replace('Root')} style={tw`mt-4 py-4`}>
				<Text style={tw`text-sm text-gray-250`}>Go to home screen!</Text>
			</TouchableOpacity>
		</View>
	);
}
