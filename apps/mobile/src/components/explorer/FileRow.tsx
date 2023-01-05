import { ExplorerItem, isVideoExt } from '@sd/client';
import React from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';

import FileThumb from './FileThumb';

type FileRowProps = {
	data: ExplorerItem;
};

const FileRow = ({ data }: FileRowProps) => {
	const isVid = isVideoExt(data.extension || '');
	return (
		<View>
			<View style={tw`flex flex-row items-center`}>
				<FileThumb
					data={data}
					kind={data.extension === 'zip' ? 'zip' : isVid ? 'video' : 'other'}
				/>
				<Text numberOfLines={1} style={tw`text-xs font-medium text-center text-ink-dull`}>
					{data?.name}
					{data?.extension && `.${data.extension}`}
				</Text>
			</View>
		</View>
	);
};

export default FileRow;
