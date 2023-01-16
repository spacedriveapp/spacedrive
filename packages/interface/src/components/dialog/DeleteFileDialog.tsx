import { useLibraryMutation } from '@sd/client';
import { Dialog } from '@sd/ui';

import { useZodForm, z } from '@sd/ui/src/forms';

interface DeleteDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	path_id: number | undefined;
}

const schema = z.object({
	// outputPath: z.string()
});

export const DeleteFileDialog = (props: DeleteDialogProps) => {
	const deleteFile = useLibraryMutation('files.deleteFiles');

	const form = useZodForm({
		schema
	});

	const onSubmit = form.handleSubmit((data) => {
		props.setOpen(false);

		props.location_id &&
			props.path_id &&
			deleteFile.mutate({
				location_id: props.location_id,
				path_id: props.path_id
			});

		form.reset();
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			open={props.open}
			setOpen={props.setOpen}
			title="Delete a file"
			description="Configure your deletion settings."
			loading={deleteFile.isLoading}
			ctaLabel="Delete"
		>
			<p>TODO: checkbox for "delete all matching files" (only if a file is selected)</p>
		</Dialog>
	);
};
