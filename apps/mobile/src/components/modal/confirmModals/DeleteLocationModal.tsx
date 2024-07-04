import { useRef } from 'react';
import { useLibraryMutation, usePlausibleEvent, useRspcLibraryContext } from '@sd/client';
import { ConfirmModal, ModalRef } from '~/components/layout/Modal';
import { toast } from '~/components/primitive/Toast';

type Props = {
	locationId: number;
	onSubmit?: () => void;
	trigger: React.ReactNode;
	triggerStyle?: string;
};

const DeleteLocationModal = ({ trigger, onSubmit, locationId, triggerStyle }: Props) => {
	const modalRef = useRef<ModalRef>(null);
	const rspc = useRspcLibraryContext();
	const submitPlausibleEvent = usePlausibleEvent();

	const { mutate: deleteLoc, isLoading: deleteLocLoading } = useLibraryMutation(
		'locations.delete',
		{
			onSuccess: () => {
				submitPlausibleEvent({ event: { type: 'locationDelete' } });
				onSubmit?.();
				toast.success('Location deleted successfully');
			},
			onError: (error) => {
				toast.error(error.message);
			},
			onSettled: () => {
				modalRef.current?.close();
				rspc.queryClient.invalidateQueries(['locations.list']);
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
			triggerStyle={triggerStyle}
			trigger={trigger}
			ctaDanger
		/>
	);
};

export default DeleteLocationModal;
