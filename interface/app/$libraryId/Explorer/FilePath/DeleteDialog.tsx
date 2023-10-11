import { useLibraryMutation, useZodForm } from '@sd/client';
import { CheckBox, Dialog, Tooltip, useDialog, UseDialogProps } from '@sd/ui';

interface Props extends UseDialogProps {
	locationId: number;
	rescan?: () => void;
	pathIds: number[];
	includesDirectorys?: boolean;
	includesFiles?: boolean;
}

export default (props: Props) => {
	const deleteFile = useLibraryMutation('files.deleteFiles');

	const form = useZodForm();

	let type = "file";

	if(props.includesDirectorys){
		type = "directory";
		if(props.includesFiles){
			type = "item";
		}
	}

	const description = `Warning: This will delete your ${type} forever, we don't have a trash can yet...`;

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
			title={"Delete a " + type}
			description={description}
			loading={deleteFile.isLoading}
			ctaLabel="Delete"
			ctaDanger
			className="w-[200px]"
		>
			<Tooltip label="Coming soon">
				<div className="flex items-center pt-2 opacity-50">
					<CheckBox disabled className="!mt-0" />
					<p className="text-sm text-ink-dull">Delete all matching {type}s</p>
				</div>
			</Tooltip>
		</Dialog>
	);
};
