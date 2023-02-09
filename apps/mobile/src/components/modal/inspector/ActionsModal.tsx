import dayjs from 'dayjs';
import { Heart } from 'phosphor-react-native';
import { useRef } from 'react';
import { Alert, Pressable, Text, View } from 'react-native';
import { ObjectKind, formatBytes, isObject, isPath, useLibraryQuery } from '@sd/client';
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

	const item = data?.item;

	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;
	const isDir = data && isPath(data) ? data.item.is_dir : false;

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
							<Pressable onPress={() => fileInfoRef.current.present()}>
								<FileThumb data={data} size={1} />
							</Pressable>
							<View style={tw`ml-2 flex-1`}>
								{/* Name + Extension */}
								<Text style={tw`text-base font-bold text-gray-200`} numberOfLines={1}>
									{item.name}
									{item.extension && `.${item.extension}`}
								</Text>
								<View style={tw`flex flex-row`}>
									<Text style={tw`text-ink-faint text-xs`}>
										{formatBytes(Number(objectData?.size_in_bytes || 0))},
									</Text>
									<Text style={tw`text-ink-faint text-xs`}>
										{' '}
										{dayjs(item.date_created).format('MMM Do YYYY')}
									</Text>
								</View>
								{/* Info pills w/ tags */}
								<View style={tw`flex flex-row flex-wrap mt-1`}>
									{/* Kind */}
									<InfoPill
										containerStyle={tw`mr-1`}
										text={isDir ? 'Folder' : ObjectKind[objectData?.kind || 0]}
									/>
									{/* Extension */}
									{item.extension && <InfoPill text={item.extension} containerStyle={tw`mr-1`} />}
									{/* TODO: What happens if I have too many? */}
									{tagsQuery.data?.map((tag) => (
										<InfoPill
											key={tag.id}
											text={tag.name}
											containerStyle={tw.style('mr-1', { backgroundColor: tag.color + 'CC' })}
											textStyle={tw`text-white`}
										/>
									))}
									<Pressable onPress={() => Alert.alert('TODO')}>
										<PlaceholderPill text={'Add Tag'} />
									</Pressable>
								</View>
							</View>
							<Pressable style={tw`mr-4`} onPress={() => Alert.alert('TODO')}>
								<Heart color="white" size={20} weight="regular" />
							</Pressable>
							{/* <Pressable style={tw`mt-0.5`} onPress={() => fileInfoRef.current.present()}>
									<Text style={tw`text-sm text-accent`}>More</Text>
								</Pressable> */}
						</View>
						{/* Divider */}
						<Divider style={tw`my-5`} />
						{/* Buttons */}
						<Text style={tw`text-ink font-bold`}>ACTIONS HERE</Text>
					</View>
				)}
			</Modal>
			<FileInfoModal ref={fileInfoRef} data={data} />
		</>
	);
};
