import React from 'react';
import { Text, View } from 'react-native';
import { ExplorerItem, isObject } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';
import { getExplorerStore } from '~/stores/explorerStore';
import FileThumb from './FileThumb';

type FileRowProps = {
	data: ExplorerItem;
};

const FileRow = ({ data }: FileRowProps) => {
	const filePath = isObject(data) ? data.item.file_paths[0] : data.item;

	return (
		<View
			style={twStyle('flex flex-row items-center px-3', {
				height: getExplorerStore().listItemSize
			})}
		>
			<FileThumb data={data} size={0.6} />
			<View style={tw`ml-3`}>
				<Text numberOfLines={1} style={tw`text-center text-xs font-medium text-ink-dull`}>
					{filePath?.name}
					{filePath?.extension && `.${filePath.extension}`}
				</Text>
			</View>
		</View>
	);
};

export default FileRow;
