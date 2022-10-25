import { useNavigation } from '@react-navigation/native';
import { Pressable, Text, View } from 'react-native';

import tw from '../../lib/tailwind';
import { SharedScreenProps } from '../../navigation/SharedScreens';
import { useFileModalStore } from '../../stores/modalStore';
import { ExplorerItem } from '../../types/bindings';
import FileThumb from './FileThumb';

type FileItemProps = {
	data: ExplorerItem;
};

const FileItem = ({ data }: FileItemProps) => {
	const { fileRef, setData } = useFileModalStore();

	const navigation = useNavigation<SharedScreenProps<'Location'>['navigation']>();

	function handlePress() {
		// 	if (!data) return;
		// 	if (data.is_dir) {
		// 		navigation.navigate('Location', { id: data.location_id });
		// 	} else {
		// 		setData(data);
		// 		fileRef.current.present();
		// 	}
	}

	return (
		<Pressable onPress={handlePress}>
			<View style={tw`w-[90px] h-[80px] items-center`}>
				<FileThumb
					data={data}
					kind={data.extension === 'zip' ? 'zip' : isVideo(data.extension) ? 'video' : 'other'}
				/>
				<View style={tw`px-1.5 py-[1px] mt-1`}>
					<Text numberOfLines={1} style={tw`text-xs font-medium text-center text-gray-300`}>
						{data?.name}
					</Text>
				</View>
			</View>
		</Pressable>
	);
};

export default FileItem;

// Copied from FileItem.tsx (interface/src/components/explorer/FileItem.tsx)
function isVideo(extension: string) {
	return [
		'avi',
		'asf',
		'mpeg',
		'mts',
		'mpe',
		'vob',
		'qt',
		'mov',
		'asf',
		'asx',
		'mjpeg',
		'ts',
		'mxf',
		'm2ts',
		'f4v',
		'wm',
		'3gp',
		'm4v',
		'wmv',
		'mp4',
		'webm',
		'flv',
		'mpg',
		'hevc',
		'ogv',
		'swf',
		'wtv'
	].includes(extension);
}
