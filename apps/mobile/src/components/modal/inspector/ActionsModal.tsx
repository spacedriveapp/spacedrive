import dayjs from 'dayjs';
import {
	Copy,
	Icon,
	Info,
	LockSimple,
	LockSimpleOpen,
	Package,
	Pencil,
	Share,
	TrashSimple
} from 'phosphor-react-native';
import { PropsWithChildren, useRef } from 'react';
import { Pressable, Text, View, ViewStyle } from 'react-native';
import FileViewer from 'react-native-file-viewer';
import {
	getIndexedItemFilePath,
	getItemObject,
	humanizeSize,
	useLibraryMutation,
	useLibraryQuery,
	useRspcContext
} from '@sd/client';
import FileThumb from '~/components/explorer/FileThumb';
import FavoriteButton from '~/components/explorer/sections/FavoriteButton';
import InfoTagPills from '~/components/explorer/sections/InfoTagPills';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { toast } from '~/components/primitive/Toast';
import { tw, twStyle } from '~/lib/tailwind';
import { useActionsModalStore } from '~/stores/modalStore';

import FileInfoModal from './FileInfoModal';
import RenameModal from './RenameModal';

type ActionsContainerProps = PropsWithChildren<{
	style?: ViewStyle;
}>;

const ActionsContainer = ({ children, style }: ActionsContainerProps) => (
	<View style={twStyle('rounded-lg border border-app-box bg-app py-3.5', style)}>{children}</View>
);

type ActionsItemProps = {
	title: string;
	icon?: Icon;
	onPress?: () => void;
	isDanger?: boolean;
};

const ActionsItem = ({ icon, onPress, title, isDanger = false }: ActionsItemProps) => {
	const Icon = icon;
	return (
		<Pressable onPress={onPress} style={tw`flex flex-row items-center justify-between px-4`}>
			<Text
				style={twStyle(
					'text-base font-medium leading-none',
					isDanger ? 'text-red-600' : 'text-ink'
				)}
			>
				{title}
			</Text>
			{Icon && <Icon color={isDanger ? 'red' : 'white'} size={22} />}
		</Pressable>
	);
};

const ActionDivider = () => <View style={tw`my-3.5 h-[0.5px] bg-app-box`} />;

export const ActionsModal = () => {
	const fileInfoRef = useRef<ModalRef>(null);
	const renameRef = useRef<ModalRef>(null);

	const { modalRef, data } = useActionsModalStore();
	const rspc = useRspcContext();

	const objectData = data && getItemObject(data);
	const filePath = data && getIndexedItemFilePath(data);

	// Open
	const updateAccessTime = useLibraryMutation('files.updateAccessTime', {
		onSuccess: () => {
			rspc.queryClient.invalidateQueries(['search.paths']);
		}
	});
	const queriedFullPath = useLibraryQuery(['files.getPath', filePath?.id ?? -1], {
		enabled: filePath != null
	});

	const deleteFile = useLibraryMutation('files.deleteFiles', {
		onSuccess: () => {
			rspc.queryClient.invalidateQueries(['search.paths']);
			modalRef.current?.dismiss();
		}
	});

	async function handleOpen() {
		const absolutePath = queriedFullPath.data;
		if (!absolutePath) return;
		try {
			await FileViewer.open(absolutePath, {
				// Android only
				showAppsSuggestions: false, // If there is not an installed app that can open the file, open the Play Store with suggested apps
				showOpenWithDialog: true // if there is more than one app that can open the file, show an Open With dialogue box
			});
			filePath &&
				filePath.object_id &&
				(await updateAccessTime.mutateAsync([filePath.object_id]).catch(console.error));
		} catch (error) {
			toast.error('Error opening object');
		}
	}

	return (
		<>
			<Modal ref={modalRef} snapPoints={['60', '90']}>
				{data && (
					<View style={tw`flex-1 px-4`}>
						<View style={tw`flex flex-row`}>
							{/* Thumbnail/Icon */}
							<Pressable
								onPress={handleOpen}
								onLongPress={() => fileInfoRef.current?.present()}
							>
								<FileThumb data={data} size={1} />
							</Pressable>
							<View style={tw`ml-2 flex-1`}>
								{/* Name + Extension */}
								<Text
									style={tw`max-w-[220px] text-base font-bold text-gray-200`}
									numberOfLines={1}
								>
									{filePath?.name}
									{filePath?.extension && `.${filePath?.extension}`}
								</Text>
								<View style={tw`flex flex-row`}>
									<Text style={tw`text-xs text-ink-faint`}>
										{`${humanizeSize(filePath?.size_in_bytes_bytes)}`},
									</Text>
									<Text style={tw`text-xs text-ink-faint`}>
										{' '}
										{dayjs(filePath?.date_created).format('MMM Do YYYY')}
									</Text>
								</View>
								<InfoTagPills data={data} />
							</View>
							{objectData && (
								<FavoriteButton style={tw`mr-1 mt-2`} data={objectData} />
							)}
						</View>
						<View />
						{/* Actions */}
						<ActionsContainer>
							<ActionsItem title="Open" onPress={handleOpen} />
							<ActionDivider />
							<ActionsItem
								icon={Info}
								title="Show Info"
								onPress={() => fileInfoRef.current?.present()}
							/>
						</ActionsContainer>
						<ActionsContainer style={tw`mt-2`}>
							<ActionsItem
								onPress={() => {
									renameRef.current?.present();
								}}
								icon={Pencil}
								title="Rename"
							/>
							<ActionDivider />
							<ActionsItem icon={Copy} title="Duplicate" />
							<ActionDivider />
							<ActionsItem icon={Share} title="Share" />
						</ActionsContainer>
						<ActionsContainer style={tw`mt-2`}>
							<ActionsItem icon={LockSimple} title="Encrypt" />
							<ActionDivider />
							<ActionsItem icon={LockSimpleOpen} title="Decrypt" />
							<ActionDivider />
							<ActionsItem icon={Package} title="Compress" />
							<ActionDivider />
							<ActionsItem
								icon={TrashSimple}
								title="Delete"
								isDanger
								onPress={async () => {
									if (filePath && filePath.location_id) {
										await deleteFile.mutateAsync({
											location_id: filePath.location_id,
											file_path_ids: [filePath.id]
										});
									}
								}}
							/>
						</ActionsContainer>
					</View>
				)}
			</Modal>
			<RenameModal ref={renameRef} />
			<FileInfoModal ref={fileInfoRef} data={data} />
		</>
	);
};
