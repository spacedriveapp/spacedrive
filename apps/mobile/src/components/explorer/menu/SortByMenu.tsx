import { ArrowDown, ArrowUp } from 'phosphor-react-native';
import { Text, View } from 'react-native';
import { FilePathOrder, MediaDataOrder, ObjectOrder } from '@sd/client';
import { Menu, MenuItem } from '~/components/primitive/Menu';
import { tw } from '~/lib/tailwind';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';

const sortOptions: Record<FieldNames, string> = {
	name: 'Name',
	dateCreated: 'Date Created',
	dateModified: 'Date Modified',
	dateIndexed: 'Date Indexed',
	sizeInBytes: 'Size',
	dateAccessed: 'Date Accessed',
	epochTime: 'Date Taken',
	kind: 'Kind'
};

// type SortByType = FilePathOrder | ObjectOrder | MediaDataOrder;
// get keys of FilePathOrder

type ExtractFieldName<T> = T extends { field: infer F } ? F : never;
type SortByKeys =
	| ExtractFieldName<FilePathOrder>
	| ExtractFieldName<ObjectOrder>
	| ExtractFieldName<MediaDataOrder>;

type ExcludeMultiple<T, K> = T extends K ? never : T;

type FieldNames = ExcludeMultiple<SortByKeys, 'object' | 'mediaData'>;

const ArrowUpIcon = () => <ArrowUp weight="bold" size={16} color={tw.color('ink-dull')} />;
const ArrowDownIcon = () => <ArrowDown weight="bold" size={16} color={tw.color('ink-dull')} />;

const SortByMenu = () => {
	// TODO: get settings from explorer store here

	const explorerStore = useExplorerStore();

	const sortDirection = explorerStore.orderDirection;
	const orderKey = explorerStore.orderKey;

	return (
		<Menu
			trigger={
				<View style={tw`flex flex-row items-center`}>
					<Text style={tw`mr-0.5 font-medium text-ink-dull`}>{sortDirection}</Text>
					{sortDirection === 'Asc' ? <ArrowUpIcon /> : <ArrowDownIcon />}
				</View>
			}
		>
			{Object.entries(sortOptions).map(([value, text]) => (
				<MenuItem
					key={value}
					icon={
						value === orderKey
							? sortDirection === 'Asc'
								? ArrowUpIcon
								: ArrowDownIcon
							: undefined
					}
					text={text}
					value={value}
					onSelect={() => {
						if (value === orderKey) {
							// pressing value again, so we change order direction
							getExplorerStore().orderDirection =
								sortDirection === 'Asc' ? 'Desc' : 'Asc';
							return;
						}
						// Reset sort direction to descending
						// sortDirection === 'Asc' && getExplorerStore().orderDirection = 'Desc';
						getExplorerStore().orderKey = value;
					}}
				/>
			))}
		</Menu>
	);
};

export default SortByMenu;
