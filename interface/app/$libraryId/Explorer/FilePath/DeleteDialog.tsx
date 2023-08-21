import { useLibraryMutation } from '@sd/client';
import { CheckBox, Dialog, Tooltip, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm } from '@sd/ui/src/forms';

interface Props extends UseDialogProps {
	locationId: number;
	pathIds: number[];
}

export default (props: Props) => {
	const deleteFile = useLibraryMutation('files.deleteFiles');

	const form = useZodForm();

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(() =>
				deleteFile.mutateAsync({
					location_id: props.locationId,
					file_path_ids: props.pathIds
				})
			)}
			dialog={useDialog(props)}
			title="Delete a file"
			description="Warning: This will delete your file forever, we don't have a trash can yet..."
			loading={deleteFile.isLoading}
			ctaLabel="Delete"
			ctaDanger
			className="w-[200px]"
		>
			<Tooltip label="Coming soon">
				<div className="flex items-center pt-2 opacity-50">
					<CheckBox disabled className="!mt-0" />
					<p className="text-sm text-ink-dull">Delete all matching files</p>
				</div>
			</Tooltip>
		</Dialog>
	);
};
