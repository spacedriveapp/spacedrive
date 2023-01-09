import { useNavigation } from '@react-navigation/native';
import { ExplorerData, ExplorerItem } from '@sd/client';
import { FlashList } from '@shopify/flash-list';
import { Rows, SquaresFour } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Pressable, View } from 'react-native';
import Layout from '~/constants/Layout';
import SortByMenu from '~/containers/menu/SortByMenu';
import tw from '~/lib/tailwind';
import { SharedScreenProps } from '~/navigation/SharedScreens';
import { useFileModalStore } from '~/stores/modalStore';
import { isPath } from '~/types/helper';

import FileItem from './FileItem';
import FileRow from './FileRow';

type ExplorerProps = {
	data: ExplorerData;
};

const GRID_NUM_COLUMNS = 3;
const GRID_ITEM_WIDTH = Layout.window.width / GRID_NUM_COLUMNS - 10;

const Explorer = ({ data }: ExplorerProps) => {
	const navigation = useNavigation<SharedScreenProps<'Location'>['navigation']>();

	const [layoutMode, setLayoutMode] = useState<'grid' | 'list'>('grid');

	useEffect(() => {
		// Set screen title to location name.
		navigation.setOptions({
			title: data?.context.name
		});
	}, [data, navigation]);

	const { fileRef, setData } = useFileModalStore();

	function handlePress(item: ExplorerItem) {
		if (isPath(item) && item.is_dir) {
			navigation.navigate('Location', { id: item.location_id });
		} else {
			setData(item);
			fileRef.current.present();
		}
	}

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
					key={layoutMode}
					numColumns={layoutMode === 'grid' ? GRID_NUM_COLUMNS : 1}
					data={data.items}
					keyExtractor={(item) => item.id.toString()}
					renderItem={({ item }) => (
						<Pressable onPress={() => handlePress(item)}>
							{layoutMode === 'grid' ? (
								<FileItem width={GRID_ITEM_WIDTH} data={item} />
							) : (
								<FileRow data={item} />
							)}
						</Pressable>
					)}
					extraData={layoutMode}
					estimatedItemSize={layoutMode === 'grid' ? GRID_ITEM_WIDTH : undefined}
				/>
			)}
		</View>
	);
};

export default Explorer;
