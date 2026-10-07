import type {File} from '@sd/ts-client';
import type {ContextMenuItem} from '../../../hooks/useContextMenu';

type CopyTarget = Pick<
	File,
	'name' | 'sd_path' | 'is_local' | 'kind' | 'extension'
>;

interface CopyActions {
	copyFiles(): void;
	writeText(text: string): Promise<void>;
	writeImage?: (path: string) => Promise<void>;
	clearFiles(): void;
	onError(error: unknown): void;
}

export function createCopyMenuItems(
	targets: CopyTarget[],
	actions: CopyActions
): ContextMenuItem[] {
	const paths = targets.flatMap((target) =>
		'Physical' in target.sd_path ? [target.sd_path.Physical.path] : []
	);
	const image = targets.length === 1 ? targets[0] : undefined;
	const canCopyImage =
		!!actions.writeImage &&
		!!image &&
		image.is_local &&
		image.kind === 'File' &&
		'Physical' in image.sd_path &&
		['png', 'jpg', 'jpeg'].includes(image.extension?.toLowerCase() ?? '');
	const copy = (write: () => Promise<void>) => async () => {
		try {
			await write();
			// Text and image copies replace a held file operation only after success.
			actions.clearFiles();
		} catch (error) {
			actions.onError(error);
		}
	};
	return [
		{
			label:
				targets.length > 1
					? `Copy ${targets.length} Files`
					: 'Copy File',
			keybindId: 'explorer.copy',
			disabled: targets.length === 0,
			onClick: actions.copyFiles
		},
		{type: 'separator'},
		{
			label: targets.length > 1 ? 'Copy Names' : 'Copy Name',
			disabled: targets.length === 0,
			onClick: copy(() =>
				actions.writeText(
					targets.map((target) => target.name).join('\n')
				)
			)
		},
		{
			label: targets.length > 1 ? 'Copy Pathnames' : 'Copy Pathname',
			disabled: targets.length === 0 || paths.length !== targets.length,
			onClick: copy(() => actions.writeText(paths.join('\n')))
		},
		...(canCopyImage
			? [
					{
						label: 'Copy Image',
						onClick: copy(() => actions.writeImage!(paths[0]!))
					}
				]
			: [])
	];
}
