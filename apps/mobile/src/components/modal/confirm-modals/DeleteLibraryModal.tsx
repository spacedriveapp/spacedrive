import { useQueryClient } from '@tanstack/react-query';
import { useBridgeMutation } from '@sd/client';
import { useRef } from 'react';
import { ModalRef, ConfirmModal } from '~/components/layout/Modal';

type Props = {
	libraryUuid: string;
	onSubmit?: () => void;
	trigger: React.ReactNode;
};

const DeleteLibraryModal = ({ trigger, onSubmit, libraryUuid }: Props) => {
	const queryClient = useQueryClient();
	const modalRef = useRef<ModalRef>(null);

	const { mutate: deleteLibrary, isLoading: deleteLibLoading } = useBridgeMutation(
		'library.delete',
		{
			onSuccess: () => {
				queryClient.invalidateQueries(['library.list']);
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
