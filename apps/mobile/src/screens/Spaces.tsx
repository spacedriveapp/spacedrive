import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { SpacesStackScreenProps } from '~/navigation/tabs/SpacesStack';

export default function SpacesScreen({ navigation }: SpacesStackScreenProps<'Spaces'>) {
	return (
		<View style={tw`items-center justify-center flex-1`}>
			<Text style={tw`text-xl font-bold text-white`}>Spaces</Text>
		</View>
	);
}
