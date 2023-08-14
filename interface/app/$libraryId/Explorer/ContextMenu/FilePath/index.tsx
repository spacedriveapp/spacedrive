import { Plus } from 'phosphor-react';
import { ExplorerItem } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { useExplorerContext } from '../../Context';
import { ExtraFn, FilePathItems, ObjectItems, SharedItems } from '../../ContextMenu';

interface Props {
	data: Extract<ExplorerItem, { type: 'Path' }>;
	extra?: ExtraFn;
}

export default ({ data, extra }: Props) => {
	const filePath = data.item;
	const { object } = filePath;

	const { parent } = useExplorerContext();

	// const keyManagerUnlocked = useLibraryQuery(['keys.isUnlocked']).data ?? false;
	// const mountedKeys = useLibraryQuery(['keys.listMounted']);
	// const hasMountedKeys = mountedKeys.data?.length ?? 0 > 0;

	return (
		<>
			<FilePathItems.OpenOrDownload filePath={filePath} />

			<SharedItems.OpenQuickView item={data} />

			<ContextMenu.Separator />

			<SharedItems.Details />

			<ContextMenu.Separator />

			<SharedItems.RevealInNativeExplorer filePath={filePath} />

			<SharedItems.Rename />

			{extra?.({
				object: filePath.object ?? undefined,
				filePath: filePath
			})}

			<SharedItems.Deselect />

			<ContextMenu.Separator />

			<SharedItems.Share />

			<ContextMenu.Separator />

			{object && <ObjectItems.AssignTag object={object} />}

			<ContextMenu.SubMenu label="More actions..." icon={Plus}>
				<FilePathItems.CopyAsPath pathOrId={filePath.id} />

				<FilePathItems.Crypto filePath={filePath} />

				<FilePathItems.Compress filePath={filePath} />

				{object && <ObjectItems.ConvertObject filePath={filePath} object={object} />}

				{parent?.type === 'Location' && (
					<FilePathItems.ParentFolderActions
						filePath={filePath}
						locationId={parent.location.id}
					/>
				)}

				<FilePathItems.SecureDelete filePath={filePath} />
			</ContextMenu.SubMenu>

			<ContextMenu.Separator />

			<FilePathItems.Delete filePath={filePath} />
		</>
	);
};
