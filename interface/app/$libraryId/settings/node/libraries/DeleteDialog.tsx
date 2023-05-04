import { useQueryClient } from '@tanstack/react-query';
import { useBridgeMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, UseDialogProps, forms, useDialog } from '@sd/ui';

const { useZodForm, z } = forms;

interface Props extends UseDialogProps {
	libraryUuid: string;
}

export default function DeleteLibraryDialog(props: Props) {
	const dialog = useDialog(props);
	const submitPlausibleEvent = usePlausibleEvent();

	const queryClient = useQueryClient();
	const deleteLib = useBridgeMutation('library.delete', {
		onSuccess: () => {
			queryClient.invalidateQueries(['library.list']);

			submitPlausibleEvent({
				event: {
					type: 'libraryDelete'
				}
			});
		}
	});

	const form = useZodForm({ schema: z.object({}) });

	const onSubmit = form.handleSubmit(() => deleteLib.mutateAsync(props.libraryUuid));

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			title="Delete Library"
			description="Deleting a library will permanently the database, the files themselves will not be deleted."
			ctaDanger
			ctaLabel="Delete"
		/>
	);
}
