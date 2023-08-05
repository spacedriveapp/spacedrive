import { Plus } from 'phosphor-react';
import { ExplorerItem } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { ExtraFn, FilePathItems, ObjectItems, SharedItems } from '..';

interface Props {
	data: Extract<ExplorerItem, { type: 'Object' }>;
	extra?: ExtraFn;
}

export default ({ data, extra }: Props) => {
	const object = data.item;
	const filePath = data.item.file_paths[0];

	return (
		<>
			{filePath && <FilePathItems.OpenOrDownload filePath={filePath} />}

			<SharedItems.OpenQuickView item={data} />

			<ContextMenu.Separator />

			<SharedItems.Details />

			<ContextMenu.Separator />

			{filePath && <SharedItems.RevealInNativeExplorer filePath={filePath} />}

			<SharedItems.Rename />

			{extra?.({
				object: object,
				filePath: filePath
			})}

			<ContextMenu.Separator />

			<SharedItems.Share />

			{(object || filePath) && <ContextMenu.Separator />}

			{object && <ObjectItems.AssignTag object={object} />}

			{filePath && (
				<ContextMenu.SubMenu label="More actions..." icon={Plus}>
					<FilePathItems.CopyAsPath pathOrId={filePath.id} />
					<FilePathItems.Crypto filePath={filePath} />
					<FilePathItems.Compress filePath={filePath} />
					<ObjectItems.ConvertObject filePath={filePath} object={object} />
				</ContextMenu.SubMenu>
			)}

			{filePath && (
				<>
					<ContextMenu.Separator />
					<FilePathItems.Delete filePath={filePath} />
				</>
			)}
		</>
	);
};
