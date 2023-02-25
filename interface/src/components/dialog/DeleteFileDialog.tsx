import { useLibraryMutation } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm, z } from '@sd/ui/src/forms';

interface DeleteDialogProps extends UseDialogProps {
	location_id: number;
	path_id: number;
}

const schema = z.object({});

export const DeleteFileDialog = (props: DeleteDialogProps) => {
	const dialog = useDialog(props);
	const deleteFile = useLibraryMutation('files.deleteFiles');
	const form = useZodForm({
		schema
	});
	const onSubmit = form.handleSubmit(() =>
		deleteFile.mutateAsync({
			location_id: props.location_id,
			path_id: props.path_id
		})
	);

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			title="Delete a file"
			description="Configure your deletion settings."
			loading={deleteFile.isLoading}
			ctaLabel="Delete"
		>
			<p>TODO: checkbox for "delete all matching files" (only if a file is selected)</p>
		</Dialog>
	);
};
