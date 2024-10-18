import dayjs from 'dayjs';
import {
	Barcode,
	CaretLeft,
	Clock,
	Cube,
	FolderOpen,
	Icon,
	SealCheck,
	Snowflake
} from 'phosphor-react-native';
import { forwardRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { getItemFilePath, getItemObject, humanizeSize, type ExplorerItem } from '@sd/client';
import FileThumb from '~/components/explorer/FileThumb';
import InfoTagPills from '~/components/explorer/sections/InfoTagPills';
import { Modal, ModalScrollView, type ModalRef } from '~/components/layout/Modal';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import { Divider } from '~/components/primitive/Divider';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';

type MetaItemProps = {
	title: string;
	value: string | number;
	icon?: Icon;
};

function MetaItem({ title, value, icon }: MetaItemProps) {
	const Icon = icon;

	return (
		<>
			<View style={tw`flex flex-row items-center`}>
				<View style={tw`w-30 flex flex-row items-center`}>
					{Icon && <Icon color="white" size={18} style={tw`mr-1`} />}
					<Text style={tw`text-sm font-medium text-white`}>{title}</Text>
				</View>
				<Text style={tw`text-sm text-gray-400`}>{value}</Text>
			</View>
			<Divider style={tw`my-3.5`} />
		</>
	);
}

type FileInfoModalProps = {
	data: ExplorerItem | null;
};

const FileInfoModal = forwardRef<ModalRef, FileInfoModalProps>((props, ref) => {
	const { data } = props;
	const modalRef = useForwardedRef(ref);
	const filePathData = data && getItemFilePath(data);
	const objectData = data && getItemObject(data);
	return (
		<Modal
			ref={modalRef}
			enableContentPanningGesture={false}
			enablePanDownToClose={false}
			snapPoints={['70']}
		>
			<VirtualizedListWrapper style={tw`flex-col p-4`} scrollEnabled={false} horizontal>
				{data && (
					<ModalScrollView>
						{/* Back Button */}
						<Pressable
							onPress={() => modalRef.current?.close()}
							style={tw`absolute left-2 z-10 rounded-full bg-app-button p-2`}
						>
							<CaretLeft color={tw.color('ink')} size={16} weight="bold" />
						</Pressable>
						<View style={tw`items-center`}>
							{/* File Icon / Name */}
							<FileThumb data={data} size={1.6} />
							<Text style={tw`text-base font-bold text-gray-200`}>
								{filePathData?.name}
							</Text>
							<InfoTagPills
								columnCount={4}
								contentContainerStyle={tw`mx-auto`}
								data={data}
								style={tw`mt-5 items-center`}
							/>
						</View>
						{/* Details */}
						<Divider style={tw`mb-4 mt-3`} />
						<>
							{/* Size */}
							<MetaItem
								title="Size"
								icon={Cube}
								value={`${humanizeSize(filePathData?.size_in_bytes_bytes)}`}
							/>
							{/* Created */}
							{data.type !== 'SpacedropPeer' && (
								<MetaItem
									icon={Clock}
									title="Created"
									value={dayjs(data.item.date_created).format('MMM Do YYYY')}
								/>
							)}

							{/* Accessed */}
							<MetaItem
								icon={FolderOpen}
								title="Accessed"
								value={
									objectData?.date_accessed
										? dayjs(objectData.date_accessed).format('MMM Do YYYY')
										: '--'
								}
							/>

							{/* Modified */}

							{filePathData && 'cas_id' in filePathData && (
								<>
									{/* Indexed */}
									<MetaItem
										icon={Barcode}
										title="Indexed"
										value={dayjs(filePathData.date_indexed).format(
											'MMM Do YYYY'
										)}
									/>
									{/* TODO: Note */}
									{filePathData.cas_id && (
										<MetaItem
											icon={Snowflake}
											title="Content ID"
											value={filePathData.cas_id}
										/>
									)}
									{/* Checksum */}
									{filePathData?.integrity_checksum && (
										<MetaItem
											icon={SealCheck}
											title="Checksum"
											value={filePathData?.integrity_checksum}
										/>
									)}
								</>
							)}
						</>
					</ModalScrollView>
				)}
			</VirtualizedListWrapper>
		</Modal>
	);
});

export default FileInfoModal;
