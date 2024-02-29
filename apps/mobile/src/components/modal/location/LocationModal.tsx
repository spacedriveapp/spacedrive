import { forwardRef } from 'react';
import { Text, View } from 'react-native';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button, FakeButton } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';

import DeleteLocationModal from '../confirmModals/DeleteLocationModal';

interface Props {
	locationId: number;
	editLocation: () => void;
}

export const LocationModal = forwardRef<ModalRef, Props>(({ locationId, editLocation }, ref) => {
	const modalRef = useForwardedRef(ref);
	return (
		<Modal ref={modalRef} snapPoints={['17']} title="Location actions">
			<View style={tw`mt-4 flex-row gap-5 px-6`}>
				<Button style={tw`flex-1 px-0`} onPress={editLocation} variant="gray">
					<Text style={tw`text-sm font-medium text-ink`}>Edit</Text>
				</Button>
				<DeleteLocationModal
					locationId={locationId}
					triggerStyle="flex-1"
					trigger={
						<FakeButton variant="danger">
							<Text style={tw`text-sm font-medium text-ink`}>Delete</Text>
						</FakeButton>
					}
				/>
			</View>
		</Modal>
	);
});
