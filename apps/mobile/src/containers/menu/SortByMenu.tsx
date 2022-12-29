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

const SortByMenu = () => {
	const [sortBy, setSortBy] = useState('name');
	const [sortDirection, setSortDirection] = useState('asc' as 'asc' | 'desc');

	return (
		<Menu
			trigger={
				<View style={tw`flex flex-row items-center`}>
					<Text style={tw`text-ink`}>{sortOptions[sortBy]}</Text>
					{sortDirection === 'asc' ? (
						<ArrowUp size={18} color={tw.color('ink-dull')} />
					) : (
						<ArrowDown size={18} color={tw.color('ink-dull')} />
					)}
				</View>
			}
		>
			{Object.entries(sortOptions).map(([value, text]) => (
				<MenuItem
					key={value}
					icon={value === sortBy && (sortDirection === 'asc' ? ArrowUp : ArrowDown)}
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
