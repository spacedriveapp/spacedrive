import { useQueryClient } from '@tanstack/react-query';
import { useRef } from 'react';
import { useBridgeMutation, usePlausibleEvent } from '@sd/client';
import { ConfirmModal, ModalRef } from '~/components/layout/Modal';

type Props = {
	libraryUuid: string;
	onSubmit?: () => void;
	trigger: React.ReactNode;
};

const DeleteLibraryModal = ({ trigger, onSubmit, libraryUuid }: Props) => {
	const queryClient = useQueryClient();
	const modalRef = useRef<ModalRef>(null);

	const submitPlausibleEvent = usePlausibleEvent();

	const { mutate: deleteLibrary, isPending: deleteLibLoading } = useBridgeMutation(
		'library.delete',
		{
			onMutate: () => {
				console.log('Deleting library');
			},
			onSuccess: () => {
				queryClient.invalidateQueries({ queryKey: ['library.list'] });
				onSubmit?.();
				submitPlausibleEvent({ event: { type: 'libraryDelete' } });
			},
			onSettled: () => {
				modalRef.current?.close();
			}
		}
	);
	return (
		<ConfirmModal
			ref={modalRef}
			title="Delete Library"
			description="Deleting a library will permanently the database, the files themselves will not be deleted."
			ctaLabel="Delete"
			ctaAction={() => deleteLibrary(libraryUuid)}
			loading={deleteLibLoading}
			trigger={trigger}
			ctaDanger
		/>
	);
};

export default DeleteLibraryModal;
