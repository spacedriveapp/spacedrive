import React from 'react';
import { Text, View } from 'react-native';
import { ExplorerItem, ObjectKind, isObject } from '@sd/client';
import tw from '~/lib/tailwind';
import { getExplorerStore } from '~/stores/explorerStore';
import FileThumb from './FileThumb';

type FileRowProps = {
	data: ExplorerItem;
};

const FileRow = ({ data }: FileRowProps) => {
	const { item } = data;

	// temp fix (will handle this on mobile-inspector branch)
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;
	const isVid = ObjectKind[objectData?.kind || 0] === 'Video';

	return (
		<View
			style={tw.style('flex flex-row items-center px-3', {
				height: getExplorerStore().listItemSize
			})}
		>
			<FileThumb
				data={data}
				kind={item.extension === 'zip' ? 'zip' : isVid ? 'video' : 'other'}
				size={0.6}
			/>
			<View style={tw`ml-3`}>
				<Text numberOfLines={1} style={tw`text-xs font-medium text-center text-ink-dull`}>
					{item?.name}
					{item?.extension && `.${item.extension}`}
				</Text>
			</View>
		</View>
	);
};

export default FileRow;
