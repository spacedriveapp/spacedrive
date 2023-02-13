import dayjs from 'dayjs';
import { Barcode, CaretLeft, Clock, Icon, Snowflake } from 'phosphor-react-native';
import { forwardRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { ExplorerItem, isObject, isPath } from '@sd/client';
import FileThumb from '~/components/explorer/FileThumb';
import InfoTagPills from '~/components/explorer/sections/InfoTagPills';
import { Modal, ModalRef, ModalScrollView } from '~/components/layout/Modal';
import Divider from '~/components/primitive/Divider';
import useForwardedRef from '~/hooks/useForwardedRef';
import tw from '~/lib/tailwind';

type MetaItemProps = {
	title: string;
	value: string;
	icon?: Icon;
};

function MetaItem({ title, value, icon }: MetaItemProps) {
	const Icon = icon;

	return (
		<View style={tw`flex flex-row items-center`}>
			<View style={tw`w-30 flex flex-row items-center`}>
				{icon && <Icon color="white" size={18} style={tw`mr-1`} />}
				<Text style={tw`text-sm font-medium text-white`}>{title}</Text>
			</View>
			<Text style={tw`text-sm text-gray-400`}>{value}</Text>
		</View>
	);
}

type FileInfoModalProps = {
	data: ExplorerItem;
};

const FileInfoModal = forwardRef<ModalRef, FileInfoModalProps>((props, ref) => {
	const { data } = props;

	const modalRef = useForwardedRef(ref);

	const item = data?.item;

	// const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;
	const filePathData = data ? (isObject(data) ? data.item.file_paths[0] : data.item) : null;

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
					<Pressable onPress={() => modalRef.current.close()} style={tw`absolute ml-4 z-10`}>
						<CaretLeft color={tw.color('accent')} size={20} weight="bold" />
					</Pressable>
					{/* File Icon / Name */}
					<View style={tw`items-center`}>
						<FileThumb data={data} size={1.6} />
						<Text style={tw`text-base font-bold text-gray-200 mt-3`}>{item.name}</Text>
						<InfoTagPills data={data} style={tw`mt-3`} />
					</View>
					{/* Details */}
					<Divider style={tw`mt-6 mb-4`} />
					<>
						{filePathData && (
							<MetaItem icon={Snowflake} title="Content ID" value={filePathData.cas_id} />
						)}
						<Divider style={tw`my-4`} />
						{filePathData && <MetaItem title="URI" value={`${filePathData.materialized_path}`} />}
						<Divider style={tw`my-4`} />

						<MetaItem
							icon={Clock}
							title="Created"
							value={dayjs(item.date_created).format('MMM Do YYYY')}
						/>
						<Divider style={tw`my-4`} />
						<MetaItem
							icon={Barcode}
							title="Indexed"
							value={dayjs(item.date_indexed).format('MMM Do YYYY')}
						/>
					</>
				</ModalScrollView>
			)}
		</Modal>
	);
});

export default FileInfoModal;
