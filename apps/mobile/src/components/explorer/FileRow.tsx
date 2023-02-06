import React from 'react';
import { Text, View } from 'react-native';
import { ExplorerItem, isVideoExt } from '@sd/client';
import tw from '~/lib/tailwind';
import { getExplorerStore } from '~/stores/explorerStore';
import FileThumb from './FileThumb';

type FileRowProps = {
	data: ExplorerItem;
};

const FileRow = ({ data }: FileRowProps) => {
	const { item } = data;

	const isVid = isVideoExt(item.extension || '');

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
				<Text numberOfLines={1} style={tw`text-center text-xs font-medium text-ink-dull`}>
					{item?.name}
					{item?.extension && `.${item.extension}`}
				</Text>
			</View>
		</View>
	);
};

export default FileRow;
