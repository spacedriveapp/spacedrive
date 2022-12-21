import { useNavigation } from '@react-navigation/native';
import { ExplorerData } from '@sd/client';
import { FlashList } from '@shopify/flash-list';
import { CaretRight } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';

import { Menu, MenuItem } from '../primitive/Menu';
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
	}, [data]);

	const [layoutMode, setLayoutMode] = useState<'grid' | 'list'>('grid');

	// TODO: Grid/List
	// TODO: Sort by (Name, Size, Date, Type)

	return (
		<View style={tw`flex-1`}>
			<Menu trigger={<Text style={tw`text-lg text-red-500`}>MENU</Text>}>
				<MenuItem icon={CaretRight} text="Name" />
				<MenuItem icon={CaretRight} text="Date" />
				<MenuItem icon={CaretRight} text="test" />
				<MenuItem icon={CaretRight} text="test" />
			</Menu>
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
