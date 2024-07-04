import { useNavigation } from '@react-navigation/native';
import {
	CircleDashed,
	Cube,
	Folder,
	Plus,
	SelectionSlash,
	Textbox,
	X
} from 'phosphor-react-native';
import { useEffect, useRef } from 'react';
import { FlatList, Pressable, Text, View } from 'react-native';
import { Icon } from '~/components/icons/Icon';
import Fade from '~/components/layout/Fade';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchStackScreenProps } from '~/navigation/SearchStack';
import {
	FilterItem as FilterItemType,
	getSearchStore,
	KindItem,
	SearchFilters,
	TagItem,
	useSearchStore
} from '~/stores/searchStore';

const FiltersBar = () => {
	const searchStore = useSearchStore();
	const navigation = useNavigation<SearchStackScreenProps<'Filters'>['navigation']>();
	const flatListRef = useRef<FlatList>(null);
	const appliedFiltersLength = Object.keys(searchStore.appliedFilters).length;

	useEffect(() => {
		// If there are applied filters, update the searchStore filters
		if (appliedFiltersLength > 0) {
			Object.assign(getSearchStore().filters, searchStore.appliedFilters);
		}
	}, [appliedFiltersLength, searchStore.appliedFilters]);

	return (
		<View
			style={tw`h-16 w-full flex-row items-center gap-4 border-t border-app-cardborder bg-app-header px-5 py-3`}
		>
			<Button
				onPress={() => navigation.navigate('Filters')}
				style={tw`border p-1.5`}
				variant="dashed"
			>
				<Plus weight="bold" size={20} color={tw.color('text-ink-dull')} />
			</Button>
			<View style={tw`relative flex-1`}>
				<Fade height={'100%'} width={30} color="app-header">
					<FlatList
						ref={flatListRef}
						showsHorizontalScrollIndicator={false}
						horizontal
						onContentSizeChange={() => {
							if (flatListRef.current && appliedFiltersLength < 2) {
								flatListRef.current.scrollToOffset({ animated: true, offset: 0 });
							}
						}}
						data={Object.entries(searchStore.appliedFilters)}
						extraData={searchStore.filters}
						keyExtractor={(item) => item[0]}
						renderItem={({ item }) => (
							<FilterItem filter={item[0] as SearchFilters} value={item[1]} />
						)}
						contentContainerStyle={tw`flex-row gap-2 pl-4 pr-4`}
					/>
				</Fade>
			</View>
		</View>
	);
};

interface FilterItemProps {
	filter: SearchFilters;
	value: any;
}

const FilterItem = ({ filter, value }: FilterItemProps) => {
	const boxStyle = tw`w-auto flex-row items-center gap-1.5 border border-app-cardborder bg-app-card p-2`;
	const filterCapital = filter.charAt(0).toUpperCase() + filter.slice(1);
	const searchStore = useSearchStore();

	// if the filter value is false or empty, return null i.e "Hidden"
	if (!value) return null;

	return (
		<View style={tw`flex-row gap-0.5`}>
			<View style={twStyle(boxStyle, 'rounded-bl-md rounded-tl-md')}>
				<FilterIcon filter={filter} size={16} color={tw.color('text-ink-dull') as string} />
				<Text style={tw`text-sm text-ink`}>{filterCapital}</Text>
			</View>
			<View style={twStyle(boxStyle, 'rounded-none')}>
				<FilterValue filter={filter} value={value} />
			</View>
			<Pressable
				hitSlop={24}
				onPress={() => searchStore.resetFilter(filter, true)}
				style={twStyle(boxStyle, 'rounded-br-md rounded-tr-md')}
			>
				<X size={16} style={tw`text-ink-dull`} />
			</Pressable>
		</View>
	);
};

interface FilterIconProps {
	filter: SearchFilters;
	color: string;
	size: number;
}

const FilterIcon = ({ filter, size, color }: FilterIconProps) => {
	switch (filter) {
		case 'tags':
			return <CircleDashed size={size} color={color} />;
		case 'kind':
			return <Cube size={size} color={color} />;
		case 'name':
			return <Textbox size={size} color={color} />;
		case 'extension':
			return <Textbox size={size} color={color} />;
		case 'hidden':
			return <SelectionSlash size={size} color={color} />;
		default:
			return <Folder size={size} color={color} />;
	}
};

interface FilterValueProps {
	filter: SearchFilters;
	value: any;
}

const FilterValue = ({ filter, value }: FilterValueProps) => {
	switch (filter) {
		case 'tags':
			return <TagView tags={value} />;
		case 'locations':
			return <LocationView locations={value} />;
		case 'kind':
			return <KindView kinds={value} />;
		case 'name':
			return (
				<NameView
					names={value.map((v: string) => {
						return v;
					})}
				/>
			);
		case 'extension':
			return (
				<ExtensionView
					extensions={value.map((v: string) => {
						return v;
					})}
				/>
			);
		case 'hidden':
			return <HiddenView hidden={value} />;
		default:
			return null;
	}
};

const HiddenView = ({ hidden }: { hidden: boolean }) =>
	hidden && <Text style={tw`text-ink-dull`}>Hidden</Text>;

const NameView = ({ names }: { names: string[] }) => (
	<Text style={tw`text-ink-dull`}>{names.length > 1 ? `${names.length} names` : names[0]}</Text>
);

const ExtensionView = ({ extensions }: { extensions: string[] }) => (
	<Text style={tw`text-ink-dull`}>
		{extensions.length > 1 ? `${extensions.length} extensions` : extensions[0]}
	</Text>
);

const TagView = ({ tags }: { tags: TagItem[] }) => (
	<View style={tw`mx-auto flex-row items-center justify-center`}>
		{tags.map((tag) => (
			<View
				key={tag.id}
				style={twStyle(`h-4.5 w-4.5 relative rounded-full border-2 border-app-card`, {
					backgroundColor: tag.color
				})}
			/>
		))}
	</View>
);

const LocationView = ({ locations }: { locations: FilterItemType[] }) => (
	<View style={tw`flex-row items-center gap-1.5`}>
		<Icon size={20} name="Folder" />
		<Text style={tw`text-ink-dull`}>
			{locations.length > 1 ? `${locations.length} locations` : locations[0]?.name}
		</Text>
	</View>
);

const KindView = ({ kinds }: { kinds: KindItem[] }) => (
	<View style={tw`flex-row gap-1.5`}>
		<View style={tw`flex-row gap-0.5`}>
			{kinds.map((kind) => (
				<View key={kind.id} style={tw`flex-row items-center gap-1`}>
					<Icon name={kind.icon} size={16} />
				</View>
			))}
		</View>
		<Text style={tw`text-ink-dull`}>
			{kinds.length > 1 ? `${kinds.length} kinds` : kinds[0]?.name}
		</Text>
	</View>
);

export default FiltersBar;
