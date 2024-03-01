import { ArrowLeft, FunnelSimple, MagnifyingGlass } from 'phosphor-react-native';
import { Suspense, useDeferredValue, useMemo, useState } from 'react';
import { ActivityIndicator, Pressable, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { getExplorerItemData, SearchFilterArgs, useCache, useLibraryQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchStackScreenProps } from '~/navigation/SearchStack';
import { getExplorerStore } from '~/stores/explorerStore';

// TODO: Animations!

const SearchScreen = ({ navigation }: SearchStackScreenProps<'SearchHome'>) => {
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
		<View
			style={twStyle('flex-1 bg-mobile-header', {
				paddingTop: top
			})}
		>
			{/* Header */}
			<View
				style={twStyle(
					'flex flex-row items-center gap-4 border-b border-app-line/50 px-5',
					{
						paddingBottom: 20
					}
				)}
			>
				{/* Back Button */}
				<Pressable
					onPress={() => {
						navigation.goBack();
					}}
				>
					<ArrowLeft size={23} color={tw.color('ink')} />
				</Pressable>
				{/* Search Input */}
				<View style={tw`flex-1 flex-row items-center gap-2`}>
					<View
						style={tw`h-10 flex-1 flex-wrap rounded-md border border-app-line/50 bg-app-box/50`}
					>
						<View style={tw`flex h-full flex-row items-center px-3`}>
							<View style={tw`mr-3`}>
								{loading ? (
									<ActivityIndicator size={'small'} color={'white'} />
								) : (
									<MagnifyingGlass
										size={20}
										weight="bold"
										color={tw.color('ink-dull')}
									/>
								)}
							</View>
							<TextInput
								value={search}
								onChangeText={(t) => setSearch(t)}
								style={tw`leading-0 flex-1 text-sm font-medium text-ink`}
								placeholder="Search all files..."
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
					<Pressable
						onPress={() => {
							navigation.navigate('Filters');
						}}
					>
						<View
							style={tw`h-10 w-10 items-center justify-center rounded-md border border-app-line/50 bg-app-box/50`}
						>
							<FunnelSimple size={20} color={tw.color('text-zinc-300')} />
						</View>
					</Pressable>
				</View>
			</View>
			{/* Content */}
			<View style={tw`flex-1`}>
				<Suspense fallback={<ActivityIndicator />}>
					<Explorer tabHeight={false} items={items} />
				</Suspense>
			</View>
		</View>
	);
};

export default SearchScreen;
