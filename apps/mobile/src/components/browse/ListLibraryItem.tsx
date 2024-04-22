import React, { ReactElement } from 'react';
import { Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

import Card from '../layout/Card';

interface CategoryProps {
	name: string;
	icon: ReactElement;
}

const ListLibraryItem = ({ name, icon }: CategoryProps) => {
	return (
		<Card style={tw`flex-row items-center justify-between gap-2 py-3`}>
			<View style={tw`flex-row items-center gap-2 px-2`}>
				{icon}
				<Text style={twStyle(`text-sm text-white`)}>{name}</Text>
			</View>
			<View
				style={tw`h-10 w-10 flex-row items-center justify-center rounded-full border border-app-lightborder/70 px-2`}
			>
				<Text style={tw`text-xs font-medium text-ink-dull`}>
					{Math.floor(Math.random() * 200)}
				</Text>
			</View>
		</Card>
	);
};

export default ListLibraryItem;
