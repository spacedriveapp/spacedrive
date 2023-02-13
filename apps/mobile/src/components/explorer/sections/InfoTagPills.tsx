import React from 'react';
import { Alert, Pressable, Text, View, ViewStyle } from 'react-native';
import { ExplorerItem, ObjectKind, isObject, isPath, useLibraryQuery } from '@sd/client';
import { InfoPill, PlaceholderPill } from '~/components/primitive/InfoPill';
import tw, { twStyle } from '~/lib/tailwind';

type Props = {
	data: ExplorerItem;
	style?: ViewStyle;
};

const InfoTagPills = ({ data, style }: Props) => {
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;

	const tagsQuery = useLibraryQuery(['tags.getForObject', objectData?.id], {
		enabled: Boolean(objectData)
	});

	const isDir = data && isPath(data) ? data.item.is_dir : false;

	const item = data?.item;

	return (
		<View style={twStyle('flex flex-row flex-wrap mt-1', style)}>
			{/* Kind */}
			<InfoPill
				containerStyle={tw`mr-1`}
				text={isDir ? 'Folder' : ObjectKind[objectData?.kind || 0]}
			/>
			{/* Extension */}
			{item.extension && <InfoPill text={item.extension} containerStyle={tw`mr-1`} />}
			{/* TODO: What happens if I have too many? */}
			{tagsQuery.data?.map((tag) => (
				<InfoPill
					key={tag.id}
					text={tag.name}
					containerStyle={tw.style('mr-1', { backgroundColor: tag.color + 'CC' })}
					textStyle={tw`text-white`}
				/>
			))}
			<Pressable onPress={() => Alert.alert('TODO')}>
				<PlaceholderPill text={'Add Tag'} />
			</Pressable>
		</View>
	);
};

export default InfoTagPills;
