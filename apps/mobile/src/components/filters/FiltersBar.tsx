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
import { SearchFilters, useSearchStore } from '~/stores/searchStore';

import Fade from '../layout/Fade';
import { Button } from '../primitive/Button';

const FiltersBar = () => {
	const { filters, appliedFilters } = useSearchStore();
	return (
		<View
			style={tw`relative h-16 w-full flex-row items-center gap-4 border-t border-app-line/50 bg-mobile-screen px-5 py-3`}
		>
			<Button style={tw`border-2 p-1.5`} variant="dashed">
				<Plus weight="bold" size={20} color={tw.color('text-ink-dull')} />
			</Button>
			<View>
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
						contentContainerStyle={tw`pr-13 flex-row gap-2`}
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
			return value.map((tag: any) => (
				<View
					key={tag.id}
					style={twStyle(`h-5 w-5 rounded-full`, {
						backgroundColor: tag.color
					})}
				/>
			));
		case 'locations':
			return (
				<Text style={tw`text-sm text-ink-dull`}>
					{value.length > 1 ? `${value.length} locations` : value[0].name}
				</Text>
			);
		case 'kind':
			return value.map((kind: any) => (
				<Text key={kind.id} style={tw`text-ink-dull`}>
					{kind.name}
				</Text>
			));
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
			return value && <Text style={tw`text-ink-dull`}>{'True'}</Text>;
		default:
			return null;
	}
};

export default FiltersBar;
