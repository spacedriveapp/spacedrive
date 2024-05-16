import { ExplorerItem, Tag, getItemFilePath, getItemObject } from '@sd/client';
import React, { useMemo } from 'react';
import { Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';
import { getExplorerStore } from '~/stores/explorerStore';

import FileThumb from './FileThumb';

type FileRowProps = {
	data: ExplorerItem;
};

const FileRow = ({ data }: FileRowProps) => {
	const filePath = getItemFilePath(data);
	const object = getItemObject(data);

	const maxTags = 3;
	const tags = useMemo(() => {
		if (!object) return [];
		return 'tags' in object ? object.tags.slice(0, maxTags) : [];
	}, [object]);

	return (
		<View
			style={twStyle('flex flex-row items-center justify-between px-3', {
				height: getExplorerStore().listItemSize
			})}
		>
			<View style={tw`flex-row items-center`}>
			<FileThumb data={data} size={0.6} />
			<View style={tw`ml-3 max-w-[68%]`}>
				<Text numberOfLines={1} style={tw`text-center text-xs font-medium text-ink-dull`}>
					{filePath?.name}
					{filePath?.extension && `.${filePath.extension}`}
				</Text>
			</View>
			</View>
			<View style={twStyle(`mr-2 flex-row`, {
				left: tags.length * 6 //for every tag we add 2px to the left,
			})}>
			{tags.map(({tag}: {tag: Tag}, idx: number) => {
				return (
					<View
					key={tag.id}
					style={twStyle(`relative h-4 w-4 rounded-full border-2 border-black`, {
						backgroundColor: tag.color!,
						right: idx * 6,
					})}
				/>
				)
				})}
			</View>
		</View>
	);
};

export default FileRow;
