import React, { ReactElement } from 'react';
import { Text } from 'react-native';
import { tw } from '~/lib/tailwind';

import Card from '../layout/Card';

interface CategoryProps {
	name: string;
	icon: ReactElement;
}

const GridLibraryItem = ({ name, icon }: CategoryProps) => {
	return (
		<Card style={tw`h-[70px] items-center justify-center`}>
			{icon}
			<Text style={tw`mt-2 text-xs text-white`}>{name}</Text>
		</Card>
	);
};

export default GridLibraryItem;
