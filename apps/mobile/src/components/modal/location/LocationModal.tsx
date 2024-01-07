import { forwardRef } from 'react';
import { Text, View } from 'react-native';
import { useLibraryMutation, usePlausibleEvent } from '@sd/client';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';

interface Props {
	locationId: number;
	editLocation: () => void;
}

export const LocationModal = forwardRef<ModalRef, Props>(({ locationId, editLocation }, ref) => {
	const modalRef = useForwardedRef(ref);
	const submitPlausibleEvent = usePlausibleEvent();
	const { mutate: deleteLoc } = useLibraryMutation('locations.delete', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'locationDelete' } });
		},
		onSettled: () => {
			modalRef.current?.close();
		}
	});
	return (
		<Modal ref={modalRef} snapPoints={['17']} title="Location actions">
			<View style={tw`mt-4 flex-row gap-5 px-6`}>
				<Button onPress={editLocation} style={tw`flex-1`} variant="gray">
					<Text style={tw`text-sm font-medium text-ink`}>Edit</Text>
				</Button>
				<Button style={tw`flex-1`} onPress={() => deleteLoc(locationId)} variant="danger">
					<Text style={tw`text-sm font-medium text-ink`}>Delete</Text>
				</Button>
			</View>
		</Modal>
	);
});
