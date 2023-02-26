import { useLibraryMutation } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm } from '@sd/ui/src/forms';

interface Propps extends UseDialogProps {
	location_id: number;
	path_id: number;
}

export default (props: Propps) => {
	const dialog = useDialog(props);
	const deleteFile = useLibraryMutation('files.deleteFiles');

	const form = useZodForm();

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
