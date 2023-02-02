import { useRef } from 'react';
import { Button, Text, View } from 'react-native';
import { ObjectKind, isObject, useLibraryQuery } from '@sd/client';
import FileThumb from '~/components/explorer/FileThumb';
import { Modal, ModalRef } from '~/components/layout/Modal';
import Divider from '~/components/primitive/Divider';
import { InfoPill, PlaceholderPill } from '~/components/primitive/InfoPill';
import tw from '~/lib/tailwind';
import { useActionsModalStore } from '~/stores/modalStore';
import FileInfoModal from './FileInfoModal';

export const ActionsModal = () => {
	const fileInfoRef = useRef<ModalRef>(null);
	const { modalRef, data } = useActionsModalStore();

	const objectData = data ? (isObject(data) ? data : data.object) : null;
	const isDir = data?.type === 'Path' ? data.is_dir : false;

	const tagsQuery = useLibraryQuery(['tags.getForObject', objectData?.id], {
		enabled: Boolean(objectData)
	});

	return (
		<>
			<Modal ref={modalRef} snapPoints={['60%', '90%']}>
				{data && (
					<View style={tw`flex-1 p-4`}>
						<View style={tw`flex flex-row items-center`}>
							{/* Thumbnail/Icon */}
							<FileThumb data={data} size={1} />
							<View style={tw`ml-2 flex-1`}>
								{/* Name + Extension */}
								<Text style={tw`text-base font-bold text-gray-200`} numberOfLines={1}>
									{data?.name}
									{data?.extension && `.${data.extension}`}
								</Text>
								{/* Info pills w/ tags */}
								<View style={tw`flex flex-row flex-wrap mt-2`}>
									{/* Kind */}
									<InfoPill
										containerStyle={tw`mr-1`}
										text={isDir ? 'Folder' : ObjectKind[objectData?.kind || 0]}
									/>
									{/* Extension */}
									{data.extension && <InfoPill text={data.extension} containerStyle={tw`mr-1`} />}
									{/* TODO: What happens if I have too many? */}
									{tagsQuery.data?.map((tag) => (
										<InfoPill
											key={tag.id}
											text={tag.name}
											containerStyle={tw.style('mr-1', { backgroundColor: tag.color + 'CC' })}
											textStyle={tw`text-white`}
										/>
									))}
									<PlaceholderPill text={'Add Tag'} />
								</View>
								{/* <Pressable style={tw`mt-0.5`} onPress={() => fileInfoRef.current.present()}>
									<Text style={tw`text-sm text-accent`}>More</Text>
								</Pressable> */}
							</View>
						</View>
						{/* Divider */}
						<Divider style={tw`my-6`} />
						{/* Buttons */}
						<Button onPress={() => modalRef.current.close()} title="Copy" color="white" />
						<Button onPress={() => modalRef.current.close()} title="Move" color="white" />
						<Button onPress={() => modalRef.current.close()} title="Share" color="white" />
						<Button onPress={() => modalRef.current.close()} title="Delete" color="white" />
					</View>
				)}
			</Modal>
			<FileInfoModal ref={fileInfoRef} data={data} />
		</>
	);
};
