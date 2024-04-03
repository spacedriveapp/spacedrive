import { ArrowLeft, DotsThreeOutline, FunnelSimple, MagnifyingGlass } from 'phosphor-react-native';
import { Suspense, useDeferredValue, useMemo, useState } from 'react';
import { ActivityIndicator, Platform, Pressable, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { SearchFilterArgs, useObjectsExplorerQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import FiltersBar from '~/components/search/filters/FiltersBar';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchStackScreenProps } from '~/navigation/SearchStack';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';
import { useSearchStore } from '~/stores/searchStore';

const SearchScreen = ({ navigation }: SearchStackScreenProps<'Search'>) => {
	const headerHeight = useSafeAreaInsets().top;
	const [loading, setLoading] = useState(false);
	const searchStore = useSearchStore();
	const explorerStore = useExplorerStore();
	const appliedFiltersLength = useMemo(
		() => Object.keys(searchStore.appliedFilters).length,
		[searchStore.appliedFilters]
	);
	const isAndroid = Platform.OS === 'android';

	const [search, setSearch] = useState('');
	const deferredSearch = useDeferredValue(search);

	const filters = useMemo(() => {
		const [name, ext] = deferredSearch.split('.');

		const filters: SearchFilterArgs[] = [];

		if (name) filters.push({ filePath: { name: { contains: name } } });
		if (ext) filters.push({ filePath: { extension: { in: [ext] } } });

		return filters;
	}, [deferredSearch]);

	const objects = useObjectsExplorerQuery({
		arg: {
			take: 30,
			filters
		},
		order: null,
		suspense: true,
		enabled: !!deferredSearch,
		onSuccess: () => getExplorerStore().resetNewThumbnails()
	});

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
					<Explorer {...objects} tabHeight={false} />
				</Suspense>
			</View>
		</View>
	);
};

export default SearchScreen;
