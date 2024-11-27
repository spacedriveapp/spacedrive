import { useQueryClient } from '@tanstack/react-query';
import { forwardRef, useState } from 'react';
import { Text, View } from 'react-native';
import { insertLibrary, useBridgeMutation, usePlausibleEvent } from '@sd/client';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import { ModalInput } from '~/components/primitive/Input';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';
import { currentLibraryStore } from '~/utils/nav';

const CreateLibraryModal = forwardRef<ModalRef, unknown>((_, ref) => {
	const modalRef = useForwardedRef(ref);

	const queryClient = useQueryClient();
	const [libName, setLibName] = useState('');

	const submitPlausibleEvent = usePlausibleEvent();

	const { mutate: createLibrary, isPending: createLibLoading } = useBridgeMutation(
		'library.create',
		{
			onSuccess: (lib) => {
				// Reset form
				setLibName('');

				// We do this instead of invalidating the query because it triggers a full app re-render??
				insertLibrary(queryClient, lib);

				// Switch to the new library
				currentLibraryStore.id = lib.uuid;

				submitPlausibleEvent({ event: { type: 'libraryCreate' } });
			},
			onSettled: () => {
				modalRef.current?.dismiss();
			}
		}
	);

	return (
		<Modal
			ref={modalRef}
			snapPoints={['30']}
			title="Create New Library"
			description="Choose a name for your new library, you can configure this and more settings
			from the library settings later on."
			onDismiss={() => {
				// Resets form onDismiss
				setLibName('');
			}}
			showCloseButton
			// Disable panning gestures
			enableHandlePanningGesture={false}
			enableContentPanningGesture={false}
		>
			<View style={tw`px-4`}>
				<ModalInput
					value={libName}
					onChangeText={(text) => setLibName(text)}
					placeholder="My Cool Library"
				/>
				<Button
					variant="accent"
					onPress={() => createLibrary({ name: libName, default_locations: null })}
					style={tw`mt-4`}
					disabled={libName.length === 0 || createLibLoading}
				>
					<Text style={tw`text-sm font-medium text-white`}>Create</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default CreateLibraryModal;
