import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { NodesStackScreenProps } from '~/navigation/tabs/NodesStack';

export default function NodesScreen({ navigation }: NodesStackScreenProps<'Nodes'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`text-ink text-xl font-bold`}>Nodes</Text>
		</View>
	);
}
