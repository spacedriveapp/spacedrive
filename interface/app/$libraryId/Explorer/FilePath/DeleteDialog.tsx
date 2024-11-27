import { useLibraryMutation, useZodForm } from '@sd/client';
import { Dialog, useDialog, UseDialogProps } from '@sd/ui';
import i18n from '~/app/I18n';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

interface Props extends UseDialogProps {
	indexedArgs?: {
		locationId: number;
		rescan?: () => void;
		pathIds: number[];
	};
	ephemeralArgs?: {
		paths: string[];
	};
	dirCount?: number;
	fileCount?: number;
}

function getWording(dirCount: number, fileCount: number) {
	let type = 'file';
	let translatedType = i18n.t('file', { count: 1 });
	let prefix = i18n.t('prefix_a');

	if (dirCount == 1 && fileCount == 0) {
		type = 'directory';
		translatedType = i18n.t('directory', { count: dirCount });
		prefix = i18n.t('prefix_a');
	}

	if (dirCount > 1 && fileCount == 0) {
		type = 'directories';
		translatedType = i18n.t('directory', { count: dirCount });
		prefix = dirCount.toString();
	}

	if (fileCount > 1 && dirCount == 0) {
		type = 'files';
		translatedType = i18n.t('file', { count: fileCount });
		prefix = fileCount.toString();
	}

	if (fileCount > 0 && dirCount > 0) {
		type = 'items';
		translatedType = i18n.t('item', { count: fileCount + dirCount });
		prefix = (fileCount + dirCount).toString();
	}

	return { type, prefix, translatedType };
}

export default (props: Props) => {
	const { t } = useLocale();
	const deleteFile = useLibraryMutation('files.deleteFiles');
	const deleteEphemeralFile = useLibraryMutation('ephemeralFiles.deleteFiles');
	const moveToTrashFile = useLibraryMutation('files.moveToTrash');
	const moveToTrashEphemeralFile = useLibraryMutation('ephemeralFiles.moveToTrash');

	const form = useZodForm();
	const { dirCount = 0, fileCount = 0, indexedArgs, ephemeralArgs } = props;

	const { type, prefix, translatedType } = getWording(dirCount, fileCount);

	const icon = type === 'file' || type === 'files' ? 'Document' : 'Folder';

	const description = t('delete_warning', { type: translatedType });

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(async () => {
				if (indexedArgs != undefined) {
					const { locationId, rescan, pathIds } = indexedArgs;
					await deleteFile.mutateAsync({
						location_id: locationId,
						file_path_ids: pathIds
					});

					rescan?.();
				}

				if (ephemeralArgs != undefined) {
					const { paths } = ephemeralArgs;
					await deleteEphemeralFile.mutateAsync(paths);
				}
			})}
			onSubmitSecond={form.handleSubmit(async () => {
				if (indexedArgs != undefined) {
					console.log(
						'DEBUG: DeleteDialog.tsx: onSubmitSecond (Move to Trash) -> Indexed Files'
					);
					const { locationId, rescan, pathIds } = indexedArgs;
					await moveToTrashFile.mutateAsync({
						location_id: locationId,
						file_path_ids: pathIds
					});

					rescan?.();
				}

				if (ephemeralArgs != undefined) {
					console.log(
						'DEBUG: DeleteDialog.tsx: onSubmitSecond (Move to Trash) -> Ephemeral Files'
					);
					const { paths } = ephemeralArgs;
					await moveToTrashEphemeralFile.mutateAsync(paths);
				}
			})}
			icon={<Icon theme="light" name={icon} size={28} />}
			dialog={useDialog(props)}
			title={t('delete_dialog_title', { prefix, type: translatedType })}
			description={description}
			loading={deleteFile.isPending}
			ctaLabel={t('delete_forever')}
			ctaSecondLabel={t('move_to_trash')}
			closeLabel={t('close')}
			ctaDanger
			className="w-[200px]"
		>
			{/* <Tooltip label={t('coming_soon')}>
				<div className="flex items-center pt-2 opacity-50">
					<CheckBox disabled className="!mt-0" />
					<p className="text-sm text-ink-dull">
						Delete all matching {type.endsWith('s') ? type : type + 's'}
					</p>
				</div>
			</Tooltip> */}
		</Dialog>
	);
};
