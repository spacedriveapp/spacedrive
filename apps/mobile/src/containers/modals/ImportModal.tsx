import { BottomSheetModal } from '@gorhom/bottom-sheet';
import React, { forwardRef } from 'react';
import { Text, View } from 'react-native';
import { ModalBackdrop, ModalHandle } from '~/components/layout/Modal';

const ImportModal = forwardRef<BottomSheetModal, unknown>((_, ref) => {
	return (
		<BottomSheetModal
			ref={ref}
			snapPoints={['60%', '90%']}
			backdropComponent={ModalBackdrop}
			handleComponent={ModalHandle}
		>
			<View>
				<Text>Import from Photos</Text>
				<Text>Import from Files</Text>
			</View>
		</BottomSheetModal>
	);
});

export default ImportModal;
