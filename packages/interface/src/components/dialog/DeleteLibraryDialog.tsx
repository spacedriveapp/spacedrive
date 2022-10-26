import { useBridgeMutation } from '@sd/client';
import { Dialog } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { PropsWithChildren, useState } from 'react';

export default function DeleteLibraryDialog(
	props: PropsWithChildren<{
		libraryUuid: string;
	}>
) {
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
			setOpen={setOpenDeleteModal}
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
