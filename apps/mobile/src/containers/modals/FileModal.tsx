import { BottomSheetModal, BottomSheetScrollView } from '@gorhom/bottom-sheet';
import { format } from 'date-fns';
import React, { useRef } from 'react';
import { Button, Pressable, Text, View } from 'react-native';
import { ChevronLeftIcon } from 'react-native-heroicons/outline';
import { useSnapshot } from 'valtio';

import FileIcon from '../../components/file/FileIcon';
import { ModalBackdrop, ModalHandle } from '../../components/layout/Modal';
import Divider from '../../components/primitive/Divider';
import tw from '../../lib/tailwind';
import { fileModalStore } from '../../stores/modalStore';

interface MetaItemProps {
	title: string;
	value: string;
}

function MetaItem({ title, value }: MetaItemProps) {
	return (
		<View>
			<Text style={tw`text-sm font-bold text-white`}>{title}</Text>
			<Text style={tw`text-sm text-gray-400 mt-1`}>{value}</Text>
		</View>
	);
}

export const FileModal = () => {
	const { fileRef, data } = useSnapshot(fileModalStore);

	const fileDetailsRef = useRef<BottomSheetModal>(null);

	return (
		<>
			<BottomSheetModal
				ref={fileRef}
				snapPoints={['60%', '90%']}
				backdropComponent={ModalBackdrop}
				handleComponent={ModalHandle}
			>
				{data && (
					<View style={tw`flex-1 p-4 bg-gray-600`}>
						{/* File Icon / Name */}
						<View style={tw`flex flex-row items-center`}>
							<FileIcon file={data} size={1.6} />
							{/* File Name, Details etc. */}
							<View style={tw`ml-2`}>
								<Text style={tw`text-base font-bold text-gray-200`}>{data?.name}</Text>
								<View style={tw`flex flex-row mt-2`}>
									<Text style={tw`text-xs text-gray-400`}>5 MB,</Text>
									<Text style={tw`ml-1 text-xs text-gray-400`}>
										{data?.extension.toUpperCase()},
									</Text>
									<Text style={tw`ml-1 text-xs text-gray-400`}>15 Aug</Text>
								</View>
								<Pressable style={tw`mt-2`} onPress={() => fileDetailsRef.current.present()}>
									<Text style={tw`text-sm text-primary-500`}>More</Text>
								</Pressable>
							</View>
						</View>
						{/* Divider */}
						<Divider style={tw`my-6`} />
						{/* Buttons */}
						<Button onPress={() => fileRef.current.close()} title="Copy" color="white" />
						<Button onPress={() => fileRef.current.close()} title="Move" color="white" />
						<Button onPress={() => fileRef.current.close()} title="Share" color="white" />
						<Button onPress={() => fileRef.current.close()} title="Delete" color="white" />
					</View>
				)}
			</BottomSheetModal>
			{/* Details Modal */}
			<BottomSheetModal
				ref={fileDetailsRef}
				enableContentPanningGesture={false}
				enablePanDownToClose={false}
				snapPoints={['70%']}
				backdropComponent={ModalBackdrop}
				handleComponent={ModalHandle}
			>
				{data && (
					<BottomSheetScrollView style={tw`flex-1 p-4 bg-gray-600`}>
						{/* Back Button */}
						<Pressable style={tw`w-full ml-4`} onPress={() => fileDetailsRef.current.close()}>
							<ChevronLeftIcon color={tw.color('primary-500')} width={20} height={20} />
						</Pressable>
						{/* File Icon / Name */}
						<View style={tw`items-center`}>
							<FileIcon file={data} size={1.8} />
							<Text style={tw`text-base font-bold text-gray-200 mt-3`}>{data.name}</Text>
						</View>
						{/* Details */}
						<Divider style={tw`mt-6 mb-4`} />
						<>
							{/* Temp, we need cas id */}
							{data?.id && <MetaItem title="Unique Content ID" value={'555555555'} />}
							<MetaItem title="URI" value={`/Users/utku/Somewhere/vite.config.js`} />
							<Divider style={tw`my-4`} />
							<MetaItem
								title="Date Created"
								value={format(new Date(data.date_created), 'MMMM Do yyyy, h:mm:ss aaa')}
							/>
							<Divider style={tw`my-4`} />
							<MetaItem
								title="Date Indexed"
								value={format(new Date(data.date_indexed), 'MMMM Do yyyy, h:mm:ss aaa')}
							/>
						</>
					</BottomSheetScrollView>
				)}
			</BottomSheetModal>
		</>
	);
};
