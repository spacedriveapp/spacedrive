import { FlashList } from '@shopify/flash-list';
import React from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { ExplorerData } from '~/types/bindings';

type Props = {
	data: ExplorerData;
};

// Technically this is just a flashlist container for Files, but named it Explorer to match the desktop version.
const Explorer = ({ data }: Props) => {
	return (
		<View>
			{data && (
				<FlashList
					data={data.items}
					keyExtractor={(item) => item.id.toString()}
					renderItem={({ item }) => (
						<Text style={tw`text-sm font-medium text-white`}>{item.name}</Text>
					)}
				/>
			)}
		</View>
	);
};

export default Explorer;
