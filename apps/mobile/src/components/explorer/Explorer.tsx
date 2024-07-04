import { useNavigation } from '@react-navigation/native';
import { FlashList } from '@shopify/flash-list';
import { UseInfiniteQueryResult } from '@tanstack/react-query';
import * as Haptics from 'expo-haptics';
import { ActivityIndicator, Pressable } from 'react-native';
import FileViewer from 'react-native-file-viewer';
import {
	getIndexedItemFilePath,
	isPath,
	SearchData,
	useLibraryMutation,
	useLibraryQuery,
	useRspcContext,
	type ExplorerItem
} from '@sd/client';
import Layout from '~/constants/Layout';
import { tw } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { useExplorerStore } from '~/stores/explorerStore';
import { useActionsModalStore } from '~/stores/modalStore';

import ScreenContainer from '../layout/ScreenContainer';
import { toast } from '../primitive/Toast';
import FileItem from './FileItem';
import FileRow from './FileRow';
import Menu from './menu/Menu';

type ExplorerProps = {
	tabHeight?: boolean;
	items: ExplorerItem[] | null;
	/** Function to fetch next page of items. */
	loadMore: () => void;
	query: UseInfiniteQueryResult<SearchData<ExplorerItem>>;
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
	const { modalRef, setData, data } = useActionsModalStore();
	const rspc = useRspcContext();

	const filePath = data && getIndexedItemFilePath(data);

	const queriedFullPath = useLibraryQuery(['files.getPath', filePath?.id ?? -1], {
		enabled: filePath != null
	});

	const updateAccessTime = useLibraryMutation('files.updateAccessTime', {
		onSuccess: () => {
			rspc.queryClient.invalidateQueries(['search.paths']);
		}
	});

	//Open file with native api
	async function handleOpen() {
		try {
			const absolutePath = (await queriedFullPath.refetch()).data;
			if (!absolutePath) return;
			await FileViewer.open(absolutePath, {
				// Android only
				showAppsSuggestions: false, // If there is not an installed app that can open the file, open the Play Store with suggested apps
				showOpenWithDialog: true // if there is more than one app that can open the file, show an Open With dialogue box
			});
			filePath &&
				filePath.object_id &&
				(await updateAccessTime.mutateAsync([filePath.object_id]).catch(console.error));
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
			await handleOpen();
		}
	}

	//Long press to show actions modal
	function handleLongPress(data: ExplorerItem) {
		Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
		setData(data);
		modalRef.current?.present();
	}

	return (
		<ScreenContainer tabHeight={props.tabHeight} scrollview={false} style={'gap-0 py-0'}>
			<Menu />
			{/* Flashlist not supporting empty centering: https://github.com/Shopify/flash-list/discussions/517
			So needs to be done this way */}
			{/* Items */}
			{props.isEmpty ? (
				props.emptyComponent
			) : (
				<FlashList
					key={store.layoutMode}
					numColumns={store.layoutMode === 'grid' ? store.gridNumColumns : 1}
					data={props.items ?? []}
					keyExtractor={(item) =>
						item.type === 'NonIndexedPath'
							? item.item.path
							: item.type === 'SpacedropPeer'
								? item.item.name
								: item.item.id.toString()
					}
					renderItem={({ item }) => (
						<Pressable
							onPress={() => handlePress(item)}
							onLongPress={() => handleLongPress(item)}
						>
							{store.layoutMode === 'grid' ? (
								<FileItem data={item} />
							) : (
								<FileRow data={item} />
							)}
						</Pressable>
					)}
					contentContainerStyle={tw`px-2 py-5`}
					extraData={store.layoutMode}
					estimatedItemSize={
						store.layoutMode === 'grid'
							? Layout.window.width / store.gridNumColumns
							: store.listItemSize
					}
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
