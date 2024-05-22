import React, { ReactElement } from 'react';
import { Pressable } from 'react-native';
import { twStyle } from '~/lib/tailwind';

import GridLibraryItem from './GridLibraryItem';
import ListLibraryItem from './ListLibraryItem';

interface CategoryProps {
	name: string;
	icon: ReactElement;
	viewStyle?: 'grid' | 'list';
}

const LibraryItem = ({ name, icon, viewStyle = 'grid' }: CategoryProps) => {
	return (
		<Pressable style={twStyle(viewStyle === 'grid' ? 'w-[23.2%]' : 'w-full')}>
			{viewStyle === 'grid' ? (
				<GridLibraryItem name={name} icon={icon} />
			) : (
				<ListLibraryItem name={name} icon={icon} />
			)}
		</Pressable>
	);
};

export default LibraryItem;
