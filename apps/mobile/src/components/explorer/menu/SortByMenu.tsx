import { ArrowDown, ArrowUp, CaretDown, Check } from 'phosphor-react-native';
import { Text, View } from 'react-native';
import { Menu, MenuItem } from '~/components/primitive/Menu';
import { tw } from '~/lib/tailwind';
import { SortOptionsType, useSearchStore } from '~/stores/searchStore';

const sortOptions = {
	none: 'None',
	name: 'Name',
	sizeInBytes: 'Size',
	dateIndexed: 'Date Indexed',
	dateCreated: 'Date Created',
	dateModified: 'Date Modified',
	dateAccessed: 'Date Accessed',
	dateTaken: 'Date Taken',
} satisfies Record<SortOptionsType['by'], string>;

const ArrowUpIcon = () => <ArrowUp weight="bold" size={16} color={tw.color('ink-dull')} />;
const ArrowDownIcon = () => <ArrowDown weight="bold" size={16} color={tw.color('ink-dull')} />;

const SortByMenu = () => {
	const searchStore = useSearchStore();
	return (
		<Menu
			triggerStyle={tw`rounded-md border border-app-inputborder p-1.5`}
			trigger={
				<View style={tw`flex flex-row items-center`}>
					<Text style={tw`mr-1 font-medium text-ink-dull`}>Sort by:</Text>
					<Text style={tw`mr-0.5 text-ink-dull`}>{sortOptions[searchStore.sort.by]}</Text>
					<CaretDown style={tw`ml-1`} weight="bold" size={16} color={tw.color('ink-dull')} />
				</View>
			}
		>
			{Object.entries(sortOptions).map(([value, text], idx) => (
				<View key={value}>
				<MenuItem
					icon={value === searchStore.sort.by ? Check : undefined}
					text={text}
					value={value}
					onSelect={() => searchStore.updateSort(value as SortOptionsType['by'])}
				/>
					{idx !== Object.keys(sortOptions).length - 1 && <View style={tw`border-b border-app-cardborder`} />}
				</View>
			))}
		</Menu>
	);
};

export default SortByMenu;
