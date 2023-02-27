import { ArrowDown, ArrowUp } from 'phosphor-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import { Menu, MenuItem } from '~/components/primitive/Menu';
import { tw } from '~/lib/tailwind';

const sortOptions = {
	name: 'Name',
	kind: 'Kind',
	favorite: 'Favorite',
	date_created: 'Date Created',
	date_modified: 'Date Modified',
	date_last_opened: 'Date Last Opened'
};

type SortByType = keyof typeof sortOptions;

const ArrowUpIcon = () => <ArrowUp weight="bold" size={16} color={tw.color('ink')} />;
const ArrowDownIcon = () => <ArrowDown weight="bold" size={16} color={tw.color('ink')} />;

const SortByMenu = () => {
	const [sortBy, setSortBy] = useState<SortByType>('name');
	const [sortDirection, setSortDirection] = useState('asc' as 'asc' | 'desc');

	return (
		<Menu
			trigger={
				<View style={tw`flex flex-row items-center`}>
					<Text style={tw`text-ink mr-0.5 font-medium`}>{sortOptions[sortBy]}</Text>
					{sortDirection === 'asc' ? <ArrowUpIcon /> : <ArrowDownIcon />}
				</View>
			}
		>
			{Object.entries(sortOptions).map(([value, text]) => (
				<MenuItem
					key={value}
					icon={
						value === sortBy ? (sortDirection === 'asc' ? ArrowUpIcon : ArrowDownIcon) : undefined
					}
					text={text}
					value={value}
					onSelect={() => {
						if (value === sortBy) {
							setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc');
							return;
						}
						// Reset sort direction to descending
						sortDirection === 'asc' && setSortDirection('desc');
						setSortBy(value as SortByType);
					}}
				/>
			))}
		</Menu>
	);
};

export default SortByMenu;
