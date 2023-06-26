import { ArrowBendUpRight, TagSimple } from 'phosphor-react';
import { FilePath, Object, ObjectKind, useLibraryMutation } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { showAlertDialog } from '~/components';
import AssignTagMenuItems from '../../AssignTagMenuItems';

export const RemoveFromRecents = ({ object }: { object: Object }) => {
	const removeFromRecents = useLibraryMutation('files.removeAccessTime');

	return (
		<>
			{object.date_accessed !== null && (
				<ContextMenu.Item
					label="Remove from recents"
					onClick={async () => {
						try {
							await removeFromRecents.mutateAsync([object.id]);
						} catch (error) {
							showAlertDialog({
								title: 'Error',
								value: `Failed to remove file from recents, due to an error: ${error}`
							});
						}
					}}
				/>
			)}
		</>
	);
};

export const AssignTag = ({ object }: { object: Object }) => (
	<ContextMenu.SubMenu label="Assign tag" icon={TagSimple}>
		<AssignTagMenuItems objectId={object.id} />
	</ContextMenu.SubMenu>
);

const ObjectConversions: Record<number, string[]> = {
	[ObjectKind.Image]: ['PNG', 'WebP', 'Gif'],
	[ObjectKind.Video]: ['MP4', 'MOV', 'AVI']
};

export const ConvertObject = ({ object, filePath }: { object: Object; filePath: FilePath }) => {
	const { kind } = object;

	return (
		<>
			{kind !== null && [ObjectKind.Image, ObjectKind.Video].includes(kind as ObjectKind) && (
				<ContextMenu.SubMenu label="Convert to" icon={ArrowBendUpRight}>
					{ObjectConversions[kind]?.map((ext) => (
						<ContextMenu.Item key={ext} label={ext} disabled />
					))}
				</ContextMenu.SubMenu>
			)}
		</>
	);
};
