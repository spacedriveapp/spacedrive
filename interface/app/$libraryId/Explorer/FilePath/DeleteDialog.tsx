import { useLibraryMutation, useZodForm } from '@sd/client';
import { CheckBox, Dialog, Tooltip, useDialog, UseDialogProps } from '@sd/ui';

interface Props extends UseDialogProps {
	locationId: number;
	rescan?: () => void;
	pathIds: number[];
	dirCount?: number;
	fileCount?: number;
}

function getWording(dirCount: number, fileCount: number) {
	let type = 'file';
	let prefix = 'a';

	if (dirCount == 1 && fileCount == 0) {
		type = 'directory';
		prefix = 'a';
	}

	if (dirCount > 1 && fileCount == 0) {
		type = 'directories';
		prefix = dirCount.toString();
	}

	if (fileCount > 1 && dirCount == 0) {
		type = 'files';
		prefix = fileCount.toString();
	}

	if (fileCount > 0 && dirCount > 0) {
		type = 'items';
		prefix = (fileCount + dirCount).toString();
	}

	return { type, prefix };
}

export default (props: Props) => {
	const deleteFile = useLibraryMutation('files.deleteFiles');

	const form = useZodForm();
	const { dirCount = 0, fileCount = 0 } = props;

	const { type, prefix } = getWording(dirCount, fileCount);

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
			title={'Delete ' + prefix + ' ' + type}
			description={description}
			loading={deleteFile.isLoading}
			ctaLabel="Delete"
			ctaDanger
			className="w-[200px]"
		>
			<Tooltip label="Coming soon">
				<div className="flex items-center pt-2 opacity-50">
					<CheckBox disabled className="!mt-0" />
					<p className="text-sm text-ink-dull">
						Delete all matching {type.endsWith('s') ? type : type + 's'}
					</p>
				</div>
			</Tooltip>
		</Dialog>
	);
};
