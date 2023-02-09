import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useBridgeMutation } from '@sd/client';
import Dialog from '~/components/layout/Dialog';

type Props = {
	libraryUuid: string;
	onSubmit?: () => void;
	children: React.ReactNode;
};

const DeleteLibraryDialog = ({ children, onSubmit, libraryUuid }: Props) => {
	const queryClient = useQueryClient();
	const [isOpen, setIsOpen] = useState(false);

	const { mutate: deleteLibrary, isLoading: deleteLibLoading } = useBridgeMutation(
		'library.delete',
		{
			onSuccess: () => {
				queryClient.invalidateQueries(['library.list']);
				onSubmit?.();
			},
			onSettled: () => {
				// Close dialog
				setIsOpen(false);
			}
		}
	);
	return (
		<Dialog
			isVisible={isOpen}
			setIsVisible={setIsOpen}
			title="Delete Library"
			description="Deleting a library will permanently the database, the files themselves will not be deleted."
			ctaLabel="Delete"
			ctaAction={() => deleteLibrary(libraryUuid)}
			loading={deleteLibLoading}
			trigger={children}
			ctaDanger
		/>
	);
};

export default DeleteLibraryDialog;
