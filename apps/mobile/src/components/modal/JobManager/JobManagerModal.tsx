import { forwardRef, useRef } from 'react';
import { Text, View } from 'react-native';
import { Modal, ModalRef } from '~/components/layout/Modal';

export const JobManagerModal = forwardRef<ModalRef, unknown>((_, ref) => {
	return (
		<Modal ref={ref} snapPoints={['60']}>
			<View>
				<Text>JobManagerModal</Text>
			</View>
		</Modal>
	);
});
