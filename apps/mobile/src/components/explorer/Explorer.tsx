import { useNavigation } from '@react-navigation/native';
import { ExplorerData } from '@sd/client';
import { FlashList } from '@shopify/flash-list';
import { Rows, SquaresFour } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Pressable, View } from 'react-native';
import SortByMenu from '~/containers/menu/SortByMenu';
import tw from '~/lib/tailwind';

import FileItem from './FileItem';

type ExplorerProps = {
	data: ExplorerData;
};

const Explorer = ({ data }: ExplorerProps) => {
	const navigation = useNavigation();

	useEffect(() => {
		// Set screen title to location name.
		navigation.setOptions({
			title: data?.context.name
		});
	}, [data, navigation]);

	const [layoutMode, setLayoutMode] = useState<'grid' | 'list'>('grid');

	return (
		<View style={tw`flex-1`}>
			{/* Header */}
			<View style={tw`flex flex-row items-center justify-between p-3`}>
				{/* Sort By */}
				<SortByMenu />
				{/* Layout (Grid/List) */}
				{layoutMode === 'grid' ? (
					<Pressable onPress={() => setLayoutMode('list')}>
						<SquaresFour color={tw.color('ink')} size={23} />
					</Pressable>
				) : (
					<Pressable onPress={() => setLayoutMode('grid')}>
						<Rows color={tw.color('ink')} size={23} />
					</Pressable>
				)}
			</View>
			{/* Items */}
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
