import { Plus } from 'phosphor-react';
import { ExplorerItem, useLibraryMutation } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { FilePathItems, ObjectItems, SharedItems } from '..';

interface Props {
	data: Extract<ExplorerItem, { type: 'Object' }>;
}

export default ({ data }: Props) => {
	const object = data.item;
	const filePath = data.item.file_paths[0];

	const getAbsolutePath = useLibraryMutation('files.locationIdToPath');

	return (
		<>
			{filePath && <FilePathItems.OpenOrDownload filePath={filePath} />}

			<SharedItems.OpenQuickView item={data} />

			<ContextMenu.Separator />

			<SharedItems.Details />

			<ContextMenu.Separator />

			{filePath && <SharedItems.RevealInNativeExplorer filePath={filePath} />}

			<SharedItems.Rename />

			<ContextMenu.Separator />

			<SharedItems.Share />

			{(object || filePath) && <ContextMenu.Separator />}

			{object && <ObjectItems.AssignTag object={object} />}

			{filePath && (
				<ContextMenu.SubMenu label="More actions..." icon={Plus}>
					<FilePathItems.CopyAsPath
						onClick={async () => {
							navigator.clipboard.writeText(
								`${await getAbsolutePath.mutateAsync({
									location_id: filePath?.location_id || -1
								})}${filePath?.materialized_path}${filePath?.name}${
									filePath?.extension ? `.${filePath?.extension}` : ''
								}`
							);
						}}
					/>
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
