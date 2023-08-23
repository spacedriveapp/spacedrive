import { ArrowBendUpRight, TagSimple } from 'phosphor-react';
import { useMemo } from 'react';
import { ObjectKind, type ObjectKindEnum, useLibraryMutation } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { showAlertDialog } from '~/components';
import AssignTagMenuItems from '~/components/AssignTagMenuItems';
import { isNonEmpty } from '~/util';
import { ConditionalItem } from '../ConditionalItem';
import { useContextMenuContext } from '../context';

export const RemoveFromRecents = new ConditionalItem({
	useCondition: () => {
		const { selectedObjects } = useContextMenuContext();

		if (!isNonEmpty(selectedObjects)) return null;

		return { selectedObjects };
	},

	Component: ({ selectedObjects }) => {
		const removeFromRecents = useLibraryMutation('files.removeAccessTime');

		return (
			<ContextMenu.Item
				label="Remove From Recents"
				onClick={async () => {
					try {
						await removeFromRecents.mutateAsync(
							selectedObjects.map((object) => object.id)
						);
					} catch (error) {
						showAlertDialog({
							title: 'Error',
							value: `Failed to remove file from recents, due to an error: ${error}`
						});
					}
				}}
			/>
		);
	}
});

export const AssignTag = new ConditionalItem({
	useCondition: () => {
		const { selectedObjects } = useContextMenuContext();
		if (!isNonEmpty(selectedObjects)) return null;

		return { selectedObjects };
	},
	Component: ({ selectedObjects }) => (
		<ContextMenu.SubMenu label="Assign tag" icon={TagSimple}>
			<AssignTagMenuItems objects={selectedObjects} />
		</ContextMenu.SubMenu>
	)
});

const ObjectConversions: Record<number, string[]> = {
	[ObjectKind.Image]: ['PNG', 'WebP', 'Gif'],
	[ObjectKind.Video]: ['MP4', 'MOV', 'AVI']
};

const ConvertableKinds = [ObjectKind.Image, ObjectKind.Video];

export const ConvertObject = new ConditionalItem({
	useCondition: () => {
		const { selectedObjects } = useContextMenuContext();

		const kinds = useMemo(() => {
			const set = new Set<ObjectKindEnum>();

			for (const o of selectedObjects) {
				if (o.kind === null || !ConvertableKinds.includes(o.kind)) break;
				set.add(o.kind);
			}

			return [...set];
		}, [selectedObjects]);

		if (!isNonEmpty(kinds) || kinds.length > 1) return null;

		const [kind] = kinds;

		return { kind };
	},
	Component: ({ kind }) => (
		<ContextMenu.SubMenu label="Convert to" icon={ArrowBendUpRight}>
			{ObjectConversions[kind]?.map((ext) => (
				<ContextMenu.Item key={ext} label={ext} disabled />
			))}
		</ContextMenu.SubMenu>
	)
});
