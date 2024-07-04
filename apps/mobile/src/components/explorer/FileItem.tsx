import { useMemo } from 'react';
import { Text, View } from 'react-native';
import { ExplorerItem, getItemFilePath, getItemObject, Tag } from '@sd/client';
import Layout from '~/constants/Layout';
import { tw, twStyle } from '~/lib/tailwind';
import { getExplorerStore } from '~/stores/explorerStore';

import FileThumb from './FileThumb';

type FileItemProps = {
	data: ExplorerItem;
};

const FileItem = ({ data }: FileItemProps) => {
	const gridItemSize = Layout.window.width / getExplorerStore().gridNumColumns;

	const filePath = getItemFilePath(data);
	const object = getItemObject(data);

	const maxTags = 3;
	const tags = useMemo(() => {
		if (!object) return [];
		return 'tags' in object ? object.tags.slice(0, maxTags) : [];
	}, [object]);

	return (
		<View
			style={twStyle('items-center', {
				width: gridItemSize,
				height: gridItemSize
			})}
		>
			<FileThumb data={data} />
			<View style={tw`mt-1 px-1.5 py-px`}>
				<Text numberOfLines={1} style={tw`text-center text-xs font-medium text-white`}>
					{filePath?.name}
					{filePath?.extension && `.${filePath.extension}`}
				</Text>
			</View>
			<View
				style={twStyle(`mx-auto flex-row justify-center pt-1.5`, {
					left: tags.length * 2 //for every tag we add 2px to the left
				})}
			>
				{tags.map(({ tag }: { tag: Tag }, idx: number) => {
					return (
						<View
							key={tag.id}
							style={twStyle(
								`relative h-3.5 w-3.5 rounded-full border-2 border-black`,
								{
									backgroundColor: tag.color!,
									right: idx * 6
								}
							)}
						/>
					);
				})}
			</View>
		</View>
	);
};

export default FileItem;
