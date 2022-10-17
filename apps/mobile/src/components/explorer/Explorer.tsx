import { FlashList } from '@shopify/flash-list';
import React from 'react';
import { Text, View } from 'react-native';
import { useLibraryQuery } from '~/hooks/rspc';
import tw from '~/lib/tailwind';

import FileItem from './FileItem';

type Props = {
	locationId: number;
	path?: string;
};

const Explorer = ({ locationId, path }: Props) => {
	const { data } = useLibraryQuery([
		'locations.getExplorerData',
		{
			location_id: locationId,
			path: path || '',
			limit: 100,
			cursor: null
		}
	]);

	return (
		<View style={tw`flex-1`}>
			<Text style={tw`text-xl font-bold text-white mt-4`}>Location id:{locationId}</Text>
			{data && (
				<FlashList
					data={data.items}
					keyExtractor={(item) => item.id.toString()}
					renderItem={({ item }) => <FileItem data={item} />}
				/>
			)}
		</View>
	);
};

export default Explorer;
