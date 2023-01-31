import { Heart, Link, Lock } from 'phosphor-react-native';
import React from 'react';
import { Text, View } from 'react-native';
import { ObjectKind, isObject } from '@sd/client';
import FileThumb from '~/components/explorer/FileThumb';
import { Modal } from '~/components/layout/Modal';
import tw from '~/lib/tailwind';
import { useInspectorModalStore } from '~/stores/modalStore';

type MetaItemProps = {
	title: string;
	value: string;
};

function MetaItem({ title, value }: MetaItemProps) {
	return (
		<View>
			<Text style={tw`text-sm font-bold text-white`}>{title}</Text>
			<Text style={tw`text-sm text-gray-400 mt-1`}>{value}</Text>
		</View>
	);
}

export const InspectorModal = () => {
	const { modalRef: fileRef, data } = useInspectorModalStore();

	// const fileDetailsRef = useRef<ModalRef>(null);

	const objectData = data ? (isObject(data) ? data : data.object) : null;
	const isDir = data?.type === 'Path' ? data.is_dir : false;

	return (
		<>
			<Modal ref={fileRef} snapPoints={['50%', '90%']}>
				{data && (
					<View style={tw`flex-1 px-4 items-center w-full`}>
						<View style={tw`my-3`}>
							<FileThumb data={data} size={2} kind={ObjectKind[objectData?.kind || 0]} />
						</View>
						<Text style={tw`text-white text-base font-bold`}>
							{data?.name}
							{data?.extension && `.${data.extension}`}
						</Text>
						{objectData && (
							<View style={tw`flex flex-row items-center`}>
								<Heart size={20} color="white" />
								<Lock size={20} color="white" />
								<Link size={20} color="white" />
							</View>
						)}
					</View>
				)}
			</Modal>
			{/* Details Modal 
			<Modal
				ref={fileDetailsRef}
				enableContentPanningGesture={false}
				enablePanDownToClose={false}
				snapPoints={['70%']}
			>
				{data && (
					<ModalScrollView style={tw`flex-1 p-4`}>
						<Pressable style={tw`w-full ml-4`} onPress={() => fileDetailsRef.current.close()}>
							<CaretLeft color={tw.color('accent')} size={20} />
						</Pressable>
						<View style={tw`items-center`}>
							<FileThumb data={data} size={1.8} />
							<Text style={tw`text-base font-bold text-gray-200 mt-3`}>{data.name}</Text>
						</View>
						<Divider style={tw`mt-6 mb-4`} />
						<>
							{data?.id && <MetaItem title="Unique Content ID" value={objectData.cas_id} />}
							<Divider style={tw`my-4`} />
							<MetaItem title="URI" value={`/Users/utku/Somewhere/vite.config.js`} />
							<Divider style={tw`my-4`} />
							<MetaItem
								title="Date Created"
								value={dayjs(data.date_created).format('MMMM Do yyyy, h:mm:ss aaa')}
							/>
							<Divider style={tw`my-4`} />
							<MetaItem
								title="Date Indexed"
								value={dayjs(data.date_indexed).format('MMMM Do yyyy, h:mm:ss aaa')}
							/>
						</>
					</ModalScrollView>
				)}
			</Modal>
		*/}
		</>
	);
};
