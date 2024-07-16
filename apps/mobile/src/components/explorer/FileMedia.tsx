import { Platform, View } from 'react-native';
import { ExplorerItem } from '@sd/client';
import Layout from '~/constants/Layout';
import { twStyle } from '~/lib/tailwind';
import { getExplorerStore } from '~/stores/explorerStore';

import FileThumb from './FileThumb';

type FileMediaProps = {
	data: ExplorerItem;
};

const FileMedia = ({ data }: FileMediaProps) => {
	const gridItemSize = Layout.window.width / getExplorerStore().mediaColumns;
	const isAndroid = Platform.OS === 'android';

	return (
		<View
			style={twStyle('items-center', {
				width: gridItemSize,
				height: gridItemSize
			})}
		>
			<FileThumb fixedSize size={isAndroid ? 95 : 97} data={data} />
		</View>
	);
};

export default FileMedia;
