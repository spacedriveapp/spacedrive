import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { NodesStackScreenProps } from '~/navigation/tabs/NodesStack';

export default function NodesScreen({ navigation }: NodesStackScreenProps<'Nodes'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl text-ink`}>Nodes</Text>
		</View>
	);
}
