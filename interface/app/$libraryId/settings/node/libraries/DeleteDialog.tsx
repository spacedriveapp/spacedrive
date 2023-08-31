import { useQueryClient } from '@tanstack/react-query';
import { useBridgeMutation, usePlausibleEvent, useZodForm } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';

interface Props extends UseDialogProps {
	libraryUuid: string;
}

export default function DeleteLibraryDialog(props: Props) {
	const submitPlausibleEvent = usePlausibleEvent();
	const queryClient = useQueryClient();

	const deleteLib = useBridgeMutation('library.delete');

	const form = useZodForm();

	const onSubmit = form.handleSubmit(async () => {
		try {
			await deleteLib.mutateAsync(props.libraryUuid);

			queryClient.invalidateQueries(['library.list']);

			submitPlausibleEvent({
				event: {
					type: 'libraryDelete'
				}
			});
		} catch (e) {
			alert(`Failed to delete library: ${e}`);
		}
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={useDialog(props)}
			title="Delete Library"
			description="Deleting a library will permanently the database, the files themselves will not be deleted."
			ctaDanger
			ctaLabel="Delete"
		/>
	);
}
