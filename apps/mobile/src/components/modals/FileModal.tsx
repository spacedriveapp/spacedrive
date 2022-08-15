import { BottomSheetModal } from '@gorhom/bottom-sheet';
import React, { useRef } from 'react';
import { Button, Text, View } from 'react-native';

import tw from '../../lib/tailwind';
import { useFileModalStore } from '../../stores/useModalStore';
import FileIcon from '../file/FileIcon';
import ModalBackdrop from './layout/ModalBackdrop';
import ModalHandle from './layout/ModalHandle';

/*
https://github.com/software-mansion/react-native-reanimated/issues/3296
https://github.com/gorhom/react-native-bottom-sheet/issues/925
https://github.com/gorhom/react-native-bottom-sheet/issues/1036

Reanimated has a bug where it sometimes doesn't animate on mount (IOS only?), doing a console.log() seems to do a re-render and fix the issue.
We can't do this for production obvs but until then they might fix it so, let's not try weird hacks for now and live with the logs.
*/

export const FileModal = () => {
	const { fileRef, data } = useFileModalStore();

	const fileDetailsRef = useRef<BottomSheetModal>(null);

	return (
		<>
			<BottomSheetModal
				ref={fileRef}
				snapPoints={['60%', '90%']}
				backdropComponent={ModalBackdrop}
				handleComponent={ModalHandle}
				// Do not remove!
				onAnimate={(from, to) => console.log(from, to)}
			>
				<View style={tw`flex-1 items-center bg-gray-600 p-4`}>
					<FileIcon file={data?.file} size={1.8} />
					<Button onPress={() => fileRef.current.close()} title="Dismiss" color="blue" />
					<Button
						onPress={() => fileDetailsRef.current.present()}
						title="Open Details Modal"
						color="blue"
					/>
				</View>
			</BottomSheetModal>
			{/* Details Modal */}
			<BottomSheetModal
				ref={fileDetailsRef}
				enableContentPanningGesture={false}
				enablePanDownToClose={false}
				snapPoints={['25%', '50%']}
				backdropComponent={ModalBackdrop}
				handleComponent={ModalHandle}
				// Do not remove!
				onAnimate={(from, to) => console.log(from, to)}
			>
				<View style={tw`flex-1 bg-gray-600 p-4`}>
					<Text>File Details 🎉</Text>
					<Button onPress={() => fileDetailsRef.current.close()} title="Close" color="purple" />
				</View>
			</BottomSheetModal>
		</>
	);
};
