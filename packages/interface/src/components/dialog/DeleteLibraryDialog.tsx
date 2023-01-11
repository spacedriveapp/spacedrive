import { useBridgeMutation } from '@sd/client';
import { Dialog } from '@sd/ui';
import { forms } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { PropsWithChildren, useState } from 'react';

const { useZodForm, z } = forms;

interface Props {
	libraryUuid: string;
}

export default function DeleteLibraryDialog(props: PropsWithChildren<Props>) {
	const [openDeleteModal, setOpenDeleteModal] = useState(false);

	const queryClient = useQueryClient();

	const deleteLib = useBridgeMutation('library.delete', {
		onSuccess: () => {
			setOpenDeleteModal(false);
			queryClient.invalidateQueries(['library.list']);
		}
	});

	const form = useZodForm({ schema: z.object({}) });

	return (
		<Dialog
			form={form}
			onSubmit={async () => {
				await deleteLib.mutateAsync(props.libraryUuid);
			}}
			open={openDeleteModal}
			setOpen={setOpenDeleteModal}
			title="Delete Library"
			description="Deleting a library will permanently the database, the files themselves will not be deleted."
			loading={deleteLib.isLoading}
			ctaDanger
			ctaLabel="Delete"
			trigger={props.children}
		/>
	);
}
