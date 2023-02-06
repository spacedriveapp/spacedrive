import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { SpacesStackScreenProps } from '~/navigation/tabs/SpacesStack';

export default function SpacesScreen({ navigation }: SpacesStackScreenProps<'Spaces'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`text-xl font-bold text-ink`}>Spaces</Text>
		</View>
	);
}
