import dayjs from 'dayjs';
import {
	Barcode,
	CaretLeft,
	CircleWavyCheck,
	Clock,
	Cube,
	Icon,
	Snowflake
} from 'phosphor-react-native';
import { forwardRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { ExplorerItem, formatBytes, isObject, useLibraryQuery } from '@sd/client';
import FileThumb from '~/components/explorer/FileThumb';
import InfoTagPills from '~/components/explorer/sections/InfoTagPills';
import { Modal, ModalRef, ModalScrollView } from '~/components/layout/Modal';
import Divider from '~/components/primitive/Divider';
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
					{icon && <Icon color="white" size={18} style={tw`mr-1`} />}
					<Text style={tw`text-sm font-medium text-white`}>{title}</Text>
				</View>
				<Text style={tw`text-sm text-gray-400`}>{value}</Text>
			</View>
			<Divider style={tw`my-3.5`} />
		</>
	);
}

type FileInfoModalProps = {
	data: ExplorerItem;
};

const FileInfoModal = forwardRef<ModalRef, FileInfoModalProps>((props, ref) => {
	const { data } = props;

	const modalRef = useForwardedRef(ref);

	const item = data?.item;

	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;
	const filePathData = data ? (isObject(data) ? data.item.file_paths[0] : data.item) : null;

	const fullObjectData = useLibraryQuery(['files.get', { id: objectData?.id || -1 }], {
		enabled: objectData?.id !== undefined
	});

	return (
		<Modal
			ref={modalRef}
			enableContentPanningGesture={false}
			enablePanDownToClose={false}
			snapPoints={['70%']}
		>
			{data && (
				<ModalScrollView style={tw`flex-1 p-4`}>
					{/* Back Button */}
					<Pressable onPress={() => modalRef.current.close()} style={tw`absolute z-10 ml-4`}>
						<CaretLeft color={tw.color('accent')} size={20} weight="bold" />
					</Pressable>
					{/* File Icon / Name */}
					<View style={tw`items-center`}>
						<FileThumb data={data} size={1.6} />
						<Text style={tw`mt-2 text-base font-bold text-gray-200`}>{item.name}</Text>
						<InfoTagPills data={data} style={tw`mt-3`} />
					</View>
					{/* Details */}
					<Divider style={tw`mt-6 mb-4`} />
					<>
						{/* Size */}
						<MetaItem
							title="Size"
							icon={Cube}
							value={formatBytes(Number(objectData?.size_in_bytes || 0))}
						/>
						{/* Duration */}
						{fullObjectData.data?.media_data?.duration_seconds && (
							<MetaItem
								title="Duration"
								value={fullObjectData.data.media_data.duration_seconds}
								icon={Clock}
							/>
						)}
						{/* Created */}
						<MetaItem
							icon={Clock}
							title="Created"
							value={dayjs(item.date_created).format('MMM Do YYYY')}
						/>
						{/* Indexed */}
						<MetaItem
							icon={Barcode}
							title="Indexed"
							value={dayjs(item.date_indexed).format('MMM Do YYYY')}
						/>

						{filePathData && (
							<>
								{/* TODO: Note */}
								<MetaItem icon={Snowflake} title="Content ID" value={filePathData.cas_id} />
								{/* Checksum */}
								{filePathData?.integrity_checksum && (
									<MetaItem
										icon={CircleWavyCheck}
										title="Checksum"
										value={filePathData?.integrity_checksum}
									/>
								)}
							</>
						)}
					</>
				</ModalScrollView>
			)}
		</Modal>
	);
});

export default FileInfoModal;
