import { MagnifyingGlass } from 'phosphor-react-native';
import { Suspense, useDeferredValue, useMemo, useState } from 'react';
import { ActivityIndicator, Pressable, Text, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { getExplorerItemData, SearchFilterArgs, useCache, useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { tw, twStyle } from '~/lib/tailwind';
import { RootStackScreenProps } from '~/navigation';
import { getExplorerStore } from '~/stores/explorerStore';

// TODO: Animations!

const SearchScreen = ({ navigation }: RootStackScreenProps<'Search'>) => {
	const { top } = useSafeAreaInsets();

	const [loading, setLoading] = useState(false);

	const [search, setSearch] = useState('');
	const deferredSearch = useDeferredValue(search);

	const filters = useMemo(() => {
		const [name, ext] = deferredSearch.split('.');

		const filters: SearchFilterArgs[] = [];

		if (name) filters.push({ filePath: { name: { contains: name } } });
		if (ext) filters.push({ filePath: { extension: { in: [ext] } } });

		return filters;
	}, [deferredSearch]);

	const query = useLibraryQuery(
		[
			'search.paths',
			{
				// ...args,
				filters,
				take: 100
			}
		],
		{
			suspense: true,
			enabled: !!deferredSearch,
			onSuccess: () => getExplorerStore().resetNewThumbnails()
		}
	);

	const pathsItemsReferences = useMemo(() => query.data?.items ?? [], [query.data]);
	const pathsItems = useCache(pathsItemsReferences);

	const items = useMemo(() => {
		// Mobile does not thave media layout
		// if (explorerStore.layoutMode !== 'media') return pathsItems;

		return (
			pathsItems?.filter((item) => {
				const { kind } = getExplorerItemData(item);
				return kind === 'Video' || kind === 'Image';
			}) ?? []
		);
	}, [pathsItems]);

	return (
		<View style={twStyle('flex-1', { marginTop: top + 10 })}>
			{/* Header */}
			<View style={tw`mx-4 flex flex-row items-center`}>
				{/* Search Input */}
				<View style={tw`mr-3 h-10 flex-1 rounded border border-app-line bg-app-overlay`}>
					<View style={tw`flex h-full flex-row items-center px-3`}>
						<View style={tw`mr-3`}>
							{loading ? (
								<ActivityIndicator size={'small'} color={'white'} />
							) : (
								<MagnifyingGlass
									size={20}
									weight="light"
									color={tw.color('ink-faint')}
								/>
							)}
						</View>
						<TextInput
							value={search}
							onChangeText={(t) => setSearch(t)}
							style={tw`flex-1 text-sm font-medium text-ink`}
							placeholder="Search"
							clearButtonMode="never" // can't change the color??
							underlineColorAndroid="transparent"
							placeholderTextColor={tw.color('ink-dull')}
							textContentType="none"
							autoFocus
							autoCapitalize="none"
							autoCorrect={false}
						/>
					</View>
				</View>
				{/* Cancel Button */}
				<Pressable onPress={() => navigation.goBack()}>
					<Text style={tw`text-accent`}>Cancel</Text>
				</Pressable>
			</View>
			{/* Content */}
			<View style={tw`flex-1`}>
				<Suspense fallback={<ActivityIndicator />}>
					<Explorer items={items} />
				</Suspense>
			</View>
		</View>
	);
};

export default SearchScreen;
