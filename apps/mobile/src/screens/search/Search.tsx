import { useIsFocused } from '@react-navigation/native';
import { usePathsExplorerQuery } from '@sd/client';
import { ArrowLeft, DotsThreeOutline, FunnelSimple } from 'phosphor-react-native';
import { Suspense, useDeferredValue, useState } from 'react';
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
	const searchStore = useSearchStore();
	const explorerStore = useExplorerStore();
	const isFocused = useIsFocused();
	const [search, setSearch] = useState('');
	const deferredSearch = useDeferredValue(search);

	const appliedFiltersLength = Object.keys(searchStore.appliedFilters).length;
	const isAndroid = Platform.OS === 'android';

	const objects = usePathsExplorerQuery({
		arg: {
			take: 30,
			filters: searchStore.mergedFilters,
		},
		enabled: isFocused && searchStore.mergedFilters.length > 1, // only fetch when screen is focused & filters are applied
		suspense: true,
		order: null,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

	// Check if there are no objects or no search
	const noObjects = objects.items?.length === 0 || !objects.items;
	const noSearch = deferredSearch.length === 0 && appliedFiltersLength === 0;

	useFiltersSearch(deferredSearch);

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
			{appliedFiltersLength > 0 && <FiltersBar/>}
			</View>
			{/* Content */}
			<View style={tw`flex-1`}>
				<Suspense fallback={<ActivityIndicator />}>
					<Explorer
					{...objects}
					isEmpty={noObjects}
					emptyComponent={
						<Empty
						icon={noSearch ? 'Search' : 'FolderNoSpace'}
						style={twStyle('flex-1 items-center justify-center border-0', {
							marginBottom: headerHeight
						})}
						textSize="text-md"
						iconSize={100}
						description={noSearch ? 'Add filters or type to search for files' : 'No files found'}
					/>
					}
					tabHeight={false} />
				</Suspense>
			</View>
		</View>
	);
};

export default SearchScreen;
