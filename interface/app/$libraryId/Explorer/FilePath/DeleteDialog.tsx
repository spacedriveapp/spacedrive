import { useLibraryMutation, useZodForm } from '@sd/client';
import { CheckBox, Dialog, Tooltip, useDialog, UseDialogProps } from '@sd/ui';

interface Props extends UseDialogProps {
	locationId: number;
	rescan?: () => void;
	pathIds: number[];
}

export default (props: Props) => {
	const deleteFile = useLibraryMutation('files.deleteFiles');

	const form = useZodForm();

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(async () => {
				await deleteFile.mutateAsync({
					location_id: props.locationId,
					file_path_ids: props.pathIds
				});

				props.rescan?.();
			})}
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
