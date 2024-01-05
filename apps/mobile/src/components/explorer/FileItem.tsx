import { Text, View } from 'react-native';
import { ExplorerItem, getItemFilePath } from '@sd/client';
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

	return (
		<View
			style={twStyle('items-center', {
				width: gridItemSize,
				height: gridItemSize
			})}
		>
			<FileThumb data={data} />
			<View style={tw`mt-1 px-1.5 py-[1px]`}>
				<Text numberOfLines={1} style={tw`text-center text-xs font-medium text-white`}>
					{filePath?.name}
					{filePath?.extension && `.${filePath.extension}`}
				</Text>
			</View>
		</View>
	);
};

export default FileItem;
