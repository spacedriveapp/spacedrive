import React, { useEffect } from 'react';
import { Text, View } from 'react-native';
import Explorer from '~/components/explorer/Explorer';
import { useLibraryQuery } from '~/hooks/rspc';
import tw from '~/lib/tailwind';
import { SharedScreenProps } from '~/navigation/SharedScreens';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: SharedScreenProps<'Location'>) {
	const { id, path } = route.params;

	useEffect(() => {
		getExplorerStore().locationId = id;
	}, [id]);

	const { data } = useLibraryQuery([
		'locations.getExplorerData',
		{
			location_id: id,
			path: path || '',
			limit: 100,
			cursor: null
		}
	]);

	return (
		<View style={tw`items-center justify-center flex-1`}>
			<Text style={tw`text-xl font-bold text-white mt-4`}>Location id:{id}</Text>
			<View style={tw`flex-1 mt-4`}>
				<Explorer data={data} />
			</View>
		</View>
	);
}
