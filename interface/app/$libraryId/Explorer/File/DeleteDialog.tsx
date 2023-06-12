import { useLibraryMutation } from '@sd/client';
import { CheckBox, Dialog, Tooltip, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm } from '@sd/ui/src/forms';

interface Propps extends UseDialogProps {
	location_id: number;
	path_id: number;
}

export default (props: Propps) => {
	const deleteFile = useLibraryMutation('files.deleteFiles');

	const form = useZodForm();

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(() =>
				deleteFile.mutateAsync({
					location_id: props.location_id,
					path_id: props.path_id
				})
			)}
			dialog={useDialog(props)}
			title="Delete a file"
			description="Configure your deletion settings."
			loading={deleteFile.isLoading}
			ctaLabel="Delete"
		>
			<Tooltip label="Coming soon">
				<div className="flex items-center opacity-50">
					<CheckBox disabled className="!mt-0" />
					<p className="text-sm text-ink-faint">Delete all matching files</p>
				</div>
			</Tooltip>
		</Dialog>
	);
};
