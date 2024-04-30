import { useIsFocused } from '@react-navigation/native';
import { SearchFilterArgs, usePathsExplorerQuery } from '@sd/client';
import { ArrowLeft, DotsThreeOutline, FunnelSimple, MagnifyingGlass } from 'phosphor-react-native';
import { Suspense, useDeferredValue, useMemo, useState } from 'react';
import { ActivityIndicator, Platform, Pressable, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import Explorer from '~/components/explorer/Explorer';
import Empty from '~/components/layout/Empty';
import FiltersBar from '~/components/search/filters/FiltersBar';
import { useFiltersSearch } from '~/hooks/useFiltersSearch';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchStackScreenProps } from '~/navigation/SearchStack';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';
import { useSearchStore } from '~/stores/searchStore';

const SearchScreen = ({ navigation }: SearchStackScreenProps<'Search'>) => {
	const headerHeight = useSafeAreaInsets().top;
	const [loading, setLoading] = useState(false);
	const {appliedFilters, mergedFilters} = useSearchStore();
	const explorerStore = useExplorerStore();
	const isFocused = useIsFocused();
	const appliedFiltersLength = useMemo(
		() => Object.keys(appliedFilters).length,
		[appliedFilters]
	);
	const isAndroid = Platform.OS === 'android';

	const [search, setSearch] = useState('');
	const deferredSearch = useDeferredValue(search);

	const filters = useMemo(() => {
		//handles names with multiple dots - makes sure the last dot is the extension separator
		const lastDotIndex = deferredSearch.lastIndexOf('.');
		const name = deferredSearch.substring(0, lastDotIndex);
		const ext = deferredSearch.substring(lastDotIndex + 1);

		const inputFilter: SearchFilterArgs[] = [];
		if (name) inputFilter.push({ filePath: { name: { contains: name } } });
		if (ext && lastDotIndex !== -1) inputFilter.push({ filePath: { extension: { in: [ext] } } });

		// merges applied filters with input filters
		return mergedFilters.concat(inputFilter);
	}, [deferredSearch, mergedFilters]);

	const objects = usePathsExplorerQuery({
		arg: {
			take: 30,
			filters
		},
		enabled: isFocused && appliedFiltersLength > 0, // only fetch when screen is focused & filters are applied
		suspense: true,
		order: null,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	useFiltersSearch();

	return (
		<View
			style={twStyle('flex-1 bg-app-header', {
				paddingTop: headerHeight + (isAndroid ? 15 : 0)
			})}
		>
			{/* Header */}
			<View style={tw`relative z-20 border-b border-app-cardborder bg-app-header`}>
				{/* Search area input container */}
				<View style={tw`flex-row items-center justify-between gap-4 px-5 pb-3`}>
					{/* Back Button */}
					<Pressable
						hitSlop={24}
						onPress={() => {
							navigation.goBack();
						}}
					>
						<ArrowLeft size={23} color={tw.color('ink')} />
					</Pressable>
					{/* Search Input */}
					<View style={tw`flex-1 flex-row items-center gap-2`}>
						<View
							style={tw`h-10 w-4/5 flex-wrap rounded-md border border-app-inputborder bg-app-input`}
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
						<Pressable onPress={() => navigation.navigate('Filters')}>
							<View
								style={tw`h-10 w-10 items-center justify-center rounded-md border border-app-inputborder bg-app-input`}
							>
								<FunnelSimple size={20} color={tw.color('text-zinc-300')} />
							</View>
						</Pressable>
					</View>
					<Pressable
						hitSlop={24}
						onPress={() => {
							getExplorerStore().toggleMenu = !explorerStore.toggleMenu;
						}}
					>
						<DotsThreeOutline
							size={24}
							color={tw.color(
								explorerStore.toggleMenu ? 'text-accent' : 'text-zinc-300'
							)}
						/>
					</Pressable>
				</View>
				{appliedFiltersLength > 0 && <FiltersBar />}
			</View>
			{/* Content */}
			<View style={tw`flex-1`}>
				<Suspense fallback={<ActivityIndicator />}>
					<Explorer
					isEmpty={appliedFiltersLength === 0 || objects.items?.length === 0}
					emptyComponent={
						<Empty
						icon="Search"
						style={twStyle('flex-1 items-center justify-center border-0', {
							marginBottom: headerHeight
						})}
						textSize="text-md"
						iconSize={84}
						description={appliedFiltersLength === 0 ? 'Add filters to search for files' : 'No files found'}
					/>
					}
					search {...objects}
					tabHeight={false} />
				</Suspense>
			</View>
		</View>
	);
};

export default SearchScreen;
