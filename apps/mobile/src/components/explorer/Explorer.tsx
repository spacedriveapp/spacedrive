import { useLibraryQuery } from '@sd/client';
import { FlashList } from '@shopify/flash-list';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';

import FileItem from './FileItem';

type ExplorerProps = {
	locationId: number;
	path?: string;
};

const Explorer = ({ locationId, path }: ExplorerProps) => {
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
			<Text style={tw`text-xl font-bold text-ink mt-4`}>Location id:{locationId}</Text>
			{data && (
				<FlashList
					data={data.items}
					keyExtractor={(item) => item.id.toString()}
					renderItem={({ item }) => <FileItem data={item} />}
					// estimatedItemSize={}
				/>
			)}
		</View>
	);
};

export default Explorer;
