import { useCallback, useState } from 'react';
import { useBridgeMutation, useBridgeQuery, useLibraryMutation, useZodForm } from '@sd/client';
import { CheckBox, Dialog, useDialog, UseDialogProps } from '@sd/ui';
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
	const [savePref, setSavePref] = useState(false);
	const deleteFile = useLibraryMutation('files.deleteFiles');
	const deleteEphemeralFile = useLibraryMutation('ephemeralFiles.deleteFiles');
	const moveToTrashFile = useLibraryMutation('files.moveToTrash');
	const moveToTrashEphemeralFile = useLibraryMutation('ephemeralFiles.moveToTrash');
	const node = useBridgeQuery(['nodeState']);
	const editNode = useBridgeMutation('nodes.edit');

	const form = useZodForm();
	const { dirCount = 0, fileCount = 0, indexedArgs, ephemeralArgs } = props;

	const { type, prefix, translatedType } = getWording(dirCount, fileCount);

	const icon = type === 'file' || type === 'files' ? 'Document' : 'Folder';

	const description = t('delete_warning', { type: translatedType });

	const saveDeletePrefs = useCallback(
		async (deletePrefs: 'Show' | 'Trash' | 'Instant') => {
			if (!savePref) return;

			await editNode.mutateAsync({
				name: node.data?.name || null,
				p2p_port: null,
				p2p_disabled: null,
				p2p_ipv6_disabled: null,
				p2p_relay_disabled: null,
				p2p_discovery: null,
				p2p_remote_access: null,
				p2p_manual_peers: null,
				delete_prefs: deletePrefs
			});
		},
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[savePref, editNode]
	);

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(async () => {
				await saveDeletePrefs('Instant');
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
				await saveDeletePrefs('Trash');
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
			<div className="flex items-center pt-2">
				<CheckBox
					checked={savePref}
					onChange={(e) => setSavePref(e.target.checked)}
					className="!mt-0"
				/>
				<p className="ml-2 text-sm text-ink-dull">Save selected option as default</p>
			</div>
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
