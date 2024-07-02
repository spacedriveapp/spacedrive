import { useMemo } from 'react';
import { FlatList, View } from 'react-native';
import { useDebounce } from 'use-debounce';
import { uint32ArrayToBigInt, useLibraryQuery } from '@sd/client';
import { IconName } from '~/components/icons/Icon';
import ScreenContainer from '~/components/layout/ScreenContainer';
import CategoryItem from '~/components/overview/CategoryItem';
import { tw } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

const CategoriesScreen = () => {
	const kinds = useLibraryQuery(['library.kindStatistics']);
	const { search } = useSearchStore();
	const [debouncedSearch] = useDebounce(search, 200);
	const filteredKinds = useMemo(
		() =>
			kinds.data?.statistics.filter((kind) =>
				kind.name?.toLowerCase().includes(debouncedSearch.toLowerCase())
			) ?? [],
		[debouncedSearch, kinds]
	);
	return (
		<ScreenContainer scrollview={false} style={tw`relative px-6 py-0`}>
			<FlatList
				data={filteredKinds
					?.sort((a, b) => {
						const aCount = Number(a.count);
						const bCount = Number(b.count);
						if (aCount === bCount) return 0;
						return aCount > bCount ? -1 : 1;
					})
					.filter((i) => i.kind !== 0)}
				numColumns={3}
				contentContainerStyle={tw`py-6`}
				keyExtractor={(item) => item.name}
				ItemSeparatorComponent={() => <View style={tw`h-2`} />}
				showsVerticalScrollIndicator={false}
				showsHorizontalScrollIndicator={false}
				renderItem={({ item }) => {
					const { kind, name, count } = item;
					let icon = name as IconName;
					switch (name) {
						case 'Code':
							icon = 'Terminal';
							break;
						case 'Unknown':
							icon = 'Undefined';
							break;
					}
					return (
						<CategoryItem
							style={'mx-1 w-[31.4%]'}
							kind={kind}
							name={name}
							icon={icon}
							items={Number(count)}
						/>
					);
				}}
			/>
		</ScreenContainer>
	);
};

export default CategoriesScreen;
