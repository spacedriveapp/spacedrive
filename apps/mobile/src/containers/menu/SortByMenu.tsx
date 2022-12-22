import { CaretRight } from 'phosphor-react-native';
import { useState } from 'react';
import { Text } from 'react-native';
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

	return (
		<Menu trigger={<Text style={tw`text-lg text-red-500`}>{sortOptions[sortBy]}</Text>}>
			{Object.entries(sortOptions).map(([value, text]) => (
				<MenuItem
					key={value}
					icon={CaretRight}
					text={text}
					value={value}
					onSelect={() => setSortBy(value)}
				/>
			))}
		</Menu>
	);
};

export default SortByMenu;
