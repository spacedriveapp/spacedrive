import { useRef } from 'react';
import { useLibraryMutation, usePlausibleEvent } from '@sd/client';
import { ConfirmModal, ModalRef } from '~/components/layout/Modal';

type Props = {
	locationId: number;
	onSubmit?: () => void;
	trigger: React.ReactNode;
};

const DeleteLocationModal = ({ trigger, onSubmit, locationId }: Props) => {
	const modalRef = useRef<ModalRef>(null);

	const submitPlausibleEvent = usePlausibleEvent();

	const { mutate: deleteLoc, isLoading: deleteLocLoading } = useLibraryMutation(
		'locations.delete',
		{
			onSuccess: () => {
				submitPlausibleEvent({ event: { type: 'locationDelete' } });
				onSubmit?.();
			},
			onSettled: () => {
				modalRef.current?.close();
			}
		}
	);
	return (
		<ConfirmModal
			ref={modalRef}
			title="Delete Location"
			description="Deleting a location will also remove all files associated with it from the Spacedrive database, the files themselves will not be deleted."
			ctaLabel="Delete"
			ctaAction={() => deleteLoc(locationId)}
			loading={deleteLocLoading}
			trigger={trigger}
			ctaDanger
		/>
	);
};

export default DeleteLocationModal;
