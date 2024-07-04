import { ArrowDown, ArrowUp, CaretDown, Check } from 'phosphor-react-native';
import { Text, View } from 'react-native';
import { Menu, MenuItem } from '~/components/primitive/Menu';
import { tw } from '~/lib/tailwind';
import { getSearchStore, SortOptionsType, useSearchStore } from '~/stores/searchStore';

const sortOptions = {
	none: 'None',
	name: 'Name',
	sizeInBytes: 'Size',
	dateIndexed: 'Date Indexed',
	dateCreated: 'Date Created',
	dateModified: 'Date Modified',
	dateAccessed: 'Date Accessed',
	dateTaken: 'Date Taken'
} satisfies Record<SortOptionsType['by'], string>;

const sortOrder = ['Asc', 'Desc'] as SortOptionsType['direction'][];

const ArrowUpIcon = (
	<ArrowUp style={tw`ml-0.5`} weight="bold" size={14} color={tw.color('ink-dull')} />
);
const ArrowDownIcon = (
	<ArrowDown style={tw`ml-0.5`} weight="bold" size={14} color={tw.color('ink-dull')} />
);

const SortByMenu = () => {
	const searchStore = useSearchStore();
	return (
		<View style={tw`flex-row items-center gap-1.5`}>
			<Menu
				containerStyle={tw`max-w-44`}
				trigger={<Trigger activeOption={sortOptions[searchStore.sort.by]} />}
			>
				{(Object.entries(sortOptions) as [[SortOptionsType['by'], string]]).map(
					([value, text], idx) => (
						<View key={value}>
							<MenuItem
								icon={value === searchStore.sort.by ? Check : undefined}
								text={text}
								onSelect={() => (getSearchStore().sort.by = value)}
							/>
							{idx !== Object.keys(sortOptions).length - 1 && (
								<View style={tw`border-b border-app-cardborder`} />
							)}
						</View>
					)
				)}
			</Menu>
			<Menu
				containerStyle={tw`max-w-40`}
				trigger={
					<Trigger
						triggerIcon={
							searchStore.sort.direction === 'Asc' ? ArrowUpIcon : ArrowDownIcon
						}
						activeOption={searchStore.sort.direction}
					/>
				}
			>
				{sortOrder.map((value, idx) => (
					<View key={value}>
						<MenuItem
							icon={value === searchStore.sort.direction ? Check : undefined}
							text={value === 'Asc' ? 'Ascending' : 'Descending'}
							onSelect={() => (getSearchStore().sort.direction = value)}
						/>
						{idx !== 1 && <View style={tw`border-b border-app-cardborder`} />}
					</View>
				))}
			</Menu>
		</View>
	);
};

interface Props {
	activeOption: string;
	triggerIcon?: React.ReactNode;
}

const Trigger = ({ activeOption, triggerIcon }: Props) => {
	return (
		<View style={tw`flex flex-row items-center rounded-md border border-app-inputborder p-1.5`}>
			<Text style={tw`mr-0.5 text-ink-dull`}>{activeOption}</Text>
			{triggerIcon ? (
				triggerIcon
			) : (
				<CaretDown
					style={tw`ml-0.5`}
					weight="bold"
					size={16}
					color={tw.color('ink-dull')}
				/>
			)}
		</View>
	);
};

export default SortByMenu;
