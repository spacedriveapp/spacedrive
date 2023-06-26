import { Plus } from 'phosphor-react';
import { ExplorerItem } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { FilePathItems, ObjectItems, SharedItems } from '../../ContextMenu';

interface Props {
	data: Extract<ExplorerItem, { type: 'Object' }>;
}

export default ({ data }: Props) => {
	const object = data.item;
	const filePath = data.item.file_paths[0];

	return (
		<>
			{filePath && <FilePathItems.OpenOrDownload filePath={filePath} />}

			<SharedItems.OpenQuickView item={data} />

			<ContextMenu.Separator />

			<SharedItems.Details />

			<ContextMenu.Separator />

			{filePath && filePath.location_id !== null && (
				<SharedItems.OpenInNativeExplorer
					locationId={filePath.location_id}
					filePath={filePath}
				/>
			)}

			<SharedItems.Rename />

			<ContextMenu.Separator />

			<SharedItems.Share />

			{(object || filePath) && <ContextMenu.Separator />}

			{object && <ObjectItems.AssignTag object={object} />}

			{filePath && (
				<ContextMenu.SubMenu label="More actions..." icon={Plus}>
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
