import { Text, View } from 'react-native';
import { useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import tw from '~/lib/tailwind';
import { SharedScreenProps } from '~/navigation/SharedScreens';

export default function TagScreen({ navigation, route }: SharedScreenProps<'Tag'>) {
	const { id } = route.params;

	const { data } = useLibraryQuery(['tags.getExplorerData', id]);

	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`text-xl font-bold text-ink`}>Tag {id}</Text>
			<Explorer data={data} />
		</View>
	);
}
