import type { ExplorerItem } from '@sd/client';

import { useNavigation } from '@react-navigation/native';
import { FlashList } from '@shopify/flash-list';
import { InfiniteData, UseInfiniteQueryResult } from '@tanstack/react-query';
import * as Haptics from 'expo-haptics';
import { useRef } from 'react';
import { ActivityIndicator } from 'react-native';
import FileViewer from 'react-native-file-viewer';

import { getIndexedItemFilePath, isPath, libraryClient, SearchData } from '@sd/client';
import Layout from '~/constants/Layout';
import { twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { useExplorerStore } from '~/stores/explorerStore';
import { useActionsModalStore } from '~/stores/modalStore';

import { ModalRef } from '../layout/Modal';
import ScreenContainer from '../layout/ScreenContainer';
import RenameModal from '../modal/inspector/RenameModal';
import { toast } from '../primitive/Toast';
import FileItem from './FileItem';
import FileMedia from './FileMedia';
import FileRow from './FileRow';
import Menu from './menu/Menu';

type ExplorerProps = {
	tabHeight?: boolean;
	items: ExplorerItem[] | null;
	/** Function to fetch next page of items. */
	loadMore: () => void;
	query: UseInfiniteQueryResult<InfiniteData<SearchData<ExplorerItem>>>;
	count?: number;
	empty?: never;
	isEmpty?: never;
};

type Props =
	| ExplorerProps
	| ({
			// isEmpty and empty are mutually exclusive
			emptyComponent: React.ReactElement; // component to show when FlashList has no data
			isEmpty: boolean; // if true - show empty component
	  } & Omit<ExplorerProps, 'empty' | 'isEmpty'>);

const Explorer = (props: Props) => {
	const navigation = useNavigation<BrowseStackScreenProps<'Location'>['navigation']>();
	const store = useExplorerStore();
	const { modalRef, setData } = useActionsModalStore();
	const renameRef = useRef<ModalRef>(null);

	//Open file with native api
	async function handleOpen(data: ExplorerItem) {
		try {
			const filePath = getIndexedItemFilePath(data);
			const absolutePath = await libraryClient.query(['files.getPath', filePath?.id ?? -1]);
			if (!absolutePath) return;
			await FileViewer.open(absolutePath, {
				// Android only
				showAppsSuggestions: false, // If there is not an installed app that can open the file, open the Play Store with suggested apps
				showOpenWithDialog: true // if there is more than one app that can open the file, show an Open With dialogue box
			});
			if (filePath && filePath.object_id)
				await libraryClient.mutation(['files.updateAccessTime', [filePath.object_id]]);
		} catch (error) {
			toast.error('Error opening object');
		}
	}

	async function handlePress(data: ExplorerItem) {
		Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
		// If it's a directory, navigate to it
		if (isPath(data) && data.item.is_dir && data.item.location_id !== null) {
			navigation.push('Location', {
				id: data.item.location_id,
				path: `${data.item.materialized_path}${data.item.name}/`
			});
		} else {
			// Open file with native api
			setData(data);
			await handleOpen(data);
		}
	}

	//Long press to show actions modal
	function handleLongPress(data: ExplorerItem) {
		Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
		setData(data);
		modalRef.current?.present();
	}

	function renameHandler(data: ExplorerItem) {
		setData(data);
		renameRef.current?.present();
	}

	return (
		<ScreenContainer tabHeight={props.tabHeight} scrollview={false} style={'gap-0 py-0'}>
			<Menu />
			<RenameModal ref={renameRef} />
			{/* Flashlist not supporting empty centering: https://github.com/Shopify/flash-list/discussions/517
			So needs to be done this way */}
			{/* Items */}
			{props.isEmpty ? (
				props.emptyComponent
			) : (
				<FlashList
					key={store.layoutMode}
					numColumns={
						store.layoutMode === 'grid'
							? store.gridNumColumns
							: store.layoutMode === 'media'
								? store.mediaColumns
								: 1
					}
					data={props.items ?? []}
					keyExtractor={item =>
						item.type === 'NonIndexedPath'
							? item.item.path
							: item.type === 'SpacedropPeer'
								? item.item.name
								: item.item.id.toString()
					}
					renderItem={({ item }) => {
						const commonProps = {
							onPress: () => handlePress(item),
							onLongPress: () => handleLongPress(item),
							data: item
						};
						return (
							<>
								{store.layoutMode === 'grid' ? (
									<FileItem
										{...commonProps}
										renameHandler={() => renameHandler(item)}
									/>
								) : store.layoutMode === 'list' ? (
									<FileRow
										{...commonProps}
										renameHandler={() => renameHandler(item)}
									/>
								) : (
									store.layoutMode === 'media' && <FileMedia {...commonProps} />
								)}
							</>
						);
					}}
					contentContainerStyle={twStyle(
						store.layoutMode !== 'media' ? 'px-2 pt-5' : 'px-0',
						store.layoutMode === 'grid' && 'pt-9'
					)}
					extraData={store.layoutMode}
					estimatedItemSize={
						store.layoutMode === 'grid'
							? Layout.window.width / store.gridNumColumns
							: store.layoutMode === 'list'
								? store.listItemSize
								: store.layoutMode === 'media'
									? Layout.window.width / store.mediaColumns
									: 100
					}
					// ItemSeparatorComponent={() => <View style={tw`p-10`}/>}
					onEndReached={() => props.loadMore?.()}
					onEndReachedThreshold={0.6}
					ListFooterComponent={
						props.query.isFetchingNextPage ? <ActivityIndicator /> : null
					}
				/>
			)}
		</ScreenContainer>
	);
};

export default Explorer;
