import { ArrowLeft, FunnelSimple, MagnifyingGlass } from 'phosphor-react-native';
import { Suspense, useDeferredValue, useMemo, useState } from 'react';
import { ActivityIndicator, Pressable, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { SearchFilterArgs, useObjectsExplorerQuery } from '@sd/client';
import Explorer from '~/components/explorer/Explorer';
import FiltersBar from '~/components/search/filters/FiltersBar';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchStackScreenProps } from '~/navigation/SearchStack';
import { getExplorerStore } from '~/stores/explorerStore';
import { useSearchStore } from '~/stores/searchStore';

const SearchScreen = ({ navigation }: SearchStackScreenProps<'Home'>) => {
	const { top } = useSafeAreaInsets();
	const [loading, setLoading] = useState(false);
	const searchStore = useSearchStore();
	const appliedFiltersLength = useMemo(
		() => Object.keys(searchStore.appliedFilters).length,
		[searchStore.appliedFilters]
	);

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
			style={twStyle('flex-1 bg-mobile-header', {
				paddingTop: top
			})}
		>
			{/* Header */}
			<View style={tw`border-b border-app-line/50`}>
				{/* Search area input container */}
				<View style={tw`flex-row items-center gap-4 px-5 pb-3`}>
					{/* Back Button */}
					<Pressable
						onPress={() => {
							navigation.goBack();
						}}
					>
						<ArrowLeft size={23} color={tw.color('ink')} />
					</Pressable>
					{/* Search Input */}
					<View style={tw`flex-row items-center flex-1 gap-2`}>
						<View
							style={tw`flex-wrap flex-1 h-10 border rounded-md border-mobile-inputborder bg-mobile-input`}
						>
							<View style={tw`flex flex-row items-center h-full px-3`}>
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
									style={tw`flex-1 text-sm font-medium leading-0 text-ink`}
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
								style={tw`items-center justify-center w-10 h-10 border rounded-md border-mobile-inputborder bg-mobile-input`}
							>
								<FunnelSimple size={20} color={tw.color('text-zinc-300')} />
							</View>
						</Pressable>
					</View>
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
