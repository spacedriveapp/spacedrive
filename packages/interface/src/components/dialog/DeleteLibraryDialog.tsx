import { useBridgeMutation } from '@sd/client';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';

import Dialog from '../layout/Dialog';

interface Props {
	children: React.ReactNode;
	libraryUuid: string;
}

export default function DeleteLibraryDialog(props: Props) {
	const [openDeleteModal, setOpenDeleteModal] = useState(false);

	const queryClient = useQueryClient();

	const { mutate: deleteLib, isLoading: libDeletePending } = useBridgeMutation('library.delete', {
		onSuccess: () => {
			setOpenDeleteModal(false);
			queryClient.invalidateQueries(['library.list']);
		}
	});

	return (
		<Dialog
			open={openDeleteModal}
			onOpenChange={setOpenDeleteModal}
			title="Delete Library"
			description="Deleting a library will permanently the database, the files themselves will not be deleted."
			ctaAction={() => {
				deleteLib(props.libraryUuid);
			}}
			loading={libDeletePending}
			ctaDanger
			ctaLabel="Delete"
			trigger={props.children}
		/>
	);
}
