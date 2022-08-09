import React from 'react';
import { ColorValue, Text, View } from 'react-native';

import BrowseLocationItem from '../components/browse/BrowseLocationItem';
import BrowseTagItem from '../components/browse/BrowseTagItem';
import CollapsibleView from '../components/layout/CollapsibleView';
import tw from '../lib/tailwind';
import { BrowseScreenProps } from '../navigation/tabs/BrowseStack';

const placeholderLocationData = [
	{
		id: 1,
		name: 'Spacedrive'
	},
	{
		id: 2,
		name: 'Classified'
	}
];
const placeholderTagsData = [
	{
		id: 1,
		name: 'Secret',
		color: tw.color('blue-500')
	},
	{
		id: 2,
		name: 'OBS',
		color: tw.color('purple-500')
	},
	{
		id: 3,
		name: 'BlackMagic',
		color: tw.color('red-500')
	}
];

const BrowseScreen = ({ navigation }: BrowseScreenProps<'Browse'>) => {
	return (
		<View style={tw`flex-1 p-4`}>
			<CollapsibleView
				title="Locations"
				titleStyle={tw`mt-5 mb-3 ml-1 text-base font-semibold text-gray-300`}
			>
				{placeholderLocationData.map((location) => (
					<BrowseLocationItem
						key={location.id}
						folderName={location.name}
						onPress={() => navigation.navigate('Location', { id: location.id })}
					/>
				))}
				{/* Add Location */}
				<View style={tw`border border-dashed rounded border-gray-450 border-opacity-60 mt-1`}>
					<Text style={tw`text-xs font-bold text-center text-gray-400 px-2 py-2`}>
						Add Location
					</Text>
				</View>
			</CollapsibleView>
			{/* Tags  */}
			<View style={tw`mt-8`} />
			<CollapsibleView
				title="Tags"
				titleStyle={tw`mt-5 mb-3 ml-1 text-base font-semibold text-gray-300`}
			>
				{placeholderTagsData.map((tag) => (
					<BrowseTagItem
						key={tag.id}
						tagName={tag.name}
						onPress={() => navigation.navigate('Tag', { id: tag.id })}
						tagColor={tag.color as ColorValue}
					/>
				))}
			</CollapsibleView>
		</View>
	);
};

export default BrowseScreen;
