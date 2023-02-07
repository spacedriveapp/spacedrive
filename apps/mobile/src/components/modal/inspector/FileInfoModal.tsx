import dayjs from 'dayjs';
import { CaretLeft } from 'phosphor-react-native';
import { forwardRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { ExplorerItem, isObject, isPath } from '@sd/client';
import FileThumb from '~/components/explorer/FileThumb';
import { Modal, ModalRef, ModalScrollView } from '~/components/layout/Modal';
import Divider from '~/components/primitive/Divider';
import useForwardedRef from '~/hooks/useForwardedRef';
import tw from '~/lib/tailwind';

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
					<Pressable style={tw`w-full ml-4`} onPress={() => modalRef.current.close()}>
						<CaretLeft color={tw.color('accent')} size={20} />
					</Pressable>
					{/* File Icon / Name */}
					<View style={tw`items-center`}>
						<FileThumb data={data} size={1.8} />
						<Text style={tw`text-base font-bold text-gray-200 mt-3`}>{item.name}</Text>
					</View>
					{/* Details */}
					<Divider style={tw`mt-6 mb-4`} />
					<>
						{filePathData && <MetaItem title="Content ID" value={filePathData.cas_id} />}
						<Divider style={tw`my-4`} />
						{filePathData && <MetaItem title="URI" value={`${filePathData.materialized_path}`} />}
						<Divider style={tw`my-4`} />

						<MetaItem title="Created" value={dayjs(item.date_created).format('MMM Do YYYY')} />
						<Divider style={tw`my-4`} />
						<MetaItem title="Indexed" value={dayjs(item.date_indexed).format('MMM Do YYYY')} />
					</>
				</ModalScrollView>
			)}
		</Modal>
	);
});

export default FileInfoModal;
