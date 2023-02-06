import { ArrowDown, ArrowUp } from 'phosphor-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import { Menu, MenuItem } from '~/components/primitive/Menu';
import tw from '~/lib/tailwind';

const sortOptions = {
	name: 'Name',
	kind: 'Kind',
	favorite: 'Favorite',
	date_created: 'Date Created',
	date_modified: 'Date Modified',
	date_last_opened: 'Date Last Opened'
};

const ArrowUpIcon = () => <ArrowUp weight="bold" size={16} color={tw.color('ink')} />;
const ArrowDownIcon = () => <ArrowDown weight="bold" size={16} color={tw.color('ink')} />;

const SortByMenu = () => {
	const [sortBy, setSortBy] = useState('name');
	const [sortDirection, setSortDirection] = useState('asc' as 'asc' | 'desc');

	return (
		<Menu
			trigger={
				<View style={tw`flex flex-row items-center`}>
					<Text style={tw`mr-0.5 font-medium text-ink`}>{sortOptions[sortBy]}</Text>
					{sortDirection === 'asc' ? <ArrowUpIcon /> : <ArrowDownIcon />}
				</View>
			}
		>
			{Object.entries(sortOptions).map(([value, text]) => (
				<MenuItem
					key={value}
					icon={value === sortBy && (sortDirection === 'asc' ? ArrowUpIcon : ArrowDownIcon)}
					text={text}
					value={value}
					onSelect={() => {
						if (value === sortBy) {
							setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc');
							return;
						}
						// Reset sort direction to descending
						sortDirection === 'asc' && setSortDirection('desc');
						setSortBy(value);
					}}
				/>
			))}
		</Menu>
	);
};

export default SortByMenu;
