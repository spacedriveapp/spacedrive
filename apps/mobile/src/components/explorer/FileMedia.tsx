import { Pressable, View } from 'react-native';
import { ExplorerItem } from '@sd/client';
import Layout from '~/constants/Layout';
import { twStyle } from '~/lib/tailwind';
import { getExplorerStore } from '~/stores/explorerStore';

import FileThumb from './FileThumb';

type FileMediaProps = {
	data: ExplorerItem;
	onPress: () => void;
	onLongPress: () => void;
};

const FileMedia = ({ data, onPress, onLongPress }: FileMediaProps) => {
	const gridItemSize = Layout.window.width / getExplorerStore().mediaColumns;

	return (
		<Pressable onPress={() => onPress()} onLongPress={() => onLongPress()}>
			<View
				style={twStyle('items-center', {
					width: gridItemSize,
					height: gridItemSize
				})}
			>
				<FileThumb mediaView data={data} />
			</View>
		</Pressable>
	);
};

export default FileMedia;
