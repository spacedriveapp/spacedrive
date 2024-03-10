import { useNavigation } from '@react-navigation/native';
import {
	CircleDashed,
	Cube,
	Folder,
	IconProps,
	Plus,
	SelectionSlash,
	Textbox,
	X
} from 'phosphor-react-native';
import { FlatList, Pressable, Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchStackScreenProps } from '~/navigation/SearchStack';
import { KindItem, SearchFilters, TagItem, useSearchStore } from '~/stores/searchStore';

import { Icon } from '../icons/Icon';
import Fade from '../layout/Fade';
import { Button } from '../primitive/Button';

const FiltersBar = () => {
	const { filters, appliedFilters } = useSearchStore();
	const navigation = useNavigation<SearchStackScreenProps<'Filters'>['navigation']>();
	return (
		<View
			style={tw`relative h-16 w-full flex-row items-center gap-4 border-t border-app-line/50 bg-mobile-screen px-5 py-3`}
		>
			<Button
				onPress={() => navigation.navigate('Filters')}
				style={tw`border-2 p-1.5`}
				variant="dashed"
			>
				<Plus weight="bold" size={20} color={tw.color('text-ink-dull')} />
			</Button>
			<View style={tw`flex-1`}>
				<Fade height={'100%'} width={30} color="mobile-screen">
					<FlatList
						showsHorizontalScrollIndicator={false}
						horizontal
						data={Object.entries(appliedFilters)}
						extraData={filters}
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
	const iconStyle = tw`text-ink-dull`;
	const boxStyle = tw`w-auto flex-row items-center gap-1.5 border border-app-line/50 bg-app-box/50 p-2`;
	const filterCapital = filter.charAt(0).toUpperCase() + filter.slice(1);
	const searchStore = useSearchStore();
	return (
		<View style={tw`flex-row gap-0.5`}>
			<View style={twStyle(boxStyle, 'rounded-bl-md rounded-tl-md')}>
				<FilterIcon
					filter={filter}
					iconProps={{
						size: 16,
						style: iconStyle
					}}
				/>
				<Text style={tw`text-sm text-ink`}>{filterCapital}</Text>
			</View>
			<View style={twStyle(boxStyle, 'rounded-none')}>
				<FilterValue filter={filter} value={value} />
			</View>
			<Pressable
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
	iconProps?: IconProps;
}

const FilterIcon = ({ filter, iconProps }: FilterIconProps) => {
	switch (filter) {
		case 'tags':
			return <CircleDashed {...iconProps} />;
		case 'kind':
			return <Cube {...iconProps} />;
		case 'name':
			return <Textbox {...iconProps} />;
		case 'extension':
			return <Textbox {...iconProps} />;
		case 'hidden':
			return <SelectionSlash {...iconProps} />;
		default:
			return <Folder {...iconProps} />;
	}
};

interface FilterValueProps {
	filter: SearchFilters;
	value: any;
}

const FilterValue = ({ filter, value }: FilterValueProps) => {
	switch (filter) {
		case 'tags':
			return value.map((tag: TagItem) => (
				<View
					key={tag.id}
					style={twStyle(`h-5 w-5 rounded-full`, {
						backgroundColor: tag.color
					})}
				/>
			));
		case 'locations':
			if (value.length === 1) {
				return (
					<View style={tw`flex-row items-center gap-1`}>
						<Icon size={20} name="Folder" />
						<Text style={tw`text-ink-dull`}>{value[0].name}</Text>
					</View>
				);
			} else {
				return (
					<View style={tw`flex-row items-center gap-1.5`}>
						<Icon size={20} name="Folder" />
						<Text style={tw`text-ink-dull`}>
							{value.length > 1 ? `${value.length} locations` : value[0].name}
						</Text>
					</View>
				);
			}
		case 'kind':
			if (value.length === 1) {
				return (
					<View style={tw`flex-row items-center gap-1`}>
						<Icon name={value[0].icon} size={16} style={tw`text-ink-dull`} />
						<Text style={tw`text-ink-dull`}>{value[0].name}</Text>
					</View>
				);
			} else {
				return (
					<View style={tw`flex-row gap-1.5`}>
						<View style={tw`flex-row gap-0.5`}>
							{value.map((k: KindItem) => {
								return (
									<View key={k.id} style={tw`flex-row items-center gap-1`}>
										<Icon name={k.icon} size={16} style={tw`text-ink-dull`} />
									</View>
								);
							})}
						</View>
						<Text style={tw`text-ink-dull`}>
							{value.length > 1 ? `${value.length} kinds` : value[0].name}
						</Text>
					</View>
				);
			}
		case 'name':
			return (
				<Text style={tw`text-ink-dull`}>
					{value.length > 1 ? `${value.length} names` : value[0]}
				</Text>
			);
		case 'extension':
			return (
				<Text style={tw`text-ink-dull`}>
					{value.length > 1 ? `${value.length} extensions` : value[0]}
				</Text>
			);
		case 'hidden':
			return value && <Text style={tw`text-ink-dull`}>Hidden</Text>;
		default:
			return null;
	}
};

export default FiltersBar;
