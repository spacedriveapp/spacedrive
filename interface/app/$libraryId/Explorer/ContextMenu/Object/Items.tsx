import { ArrowBendUpRight, TagSimple } from '@phosphor-icons/react';
import { useMemo } from 'react';
import { ExplorerItem, ObjectKind, useLibraryMutation, type ObjectKindEnum } from '@sd/client';
import { ContextMenu, toast } from '@sd/ui';
import { Menu } from '~/components/Menu';
import { useLocale } from '~/hooks';
import { isNonEmpty } from '~/util';

import AssignTagMenuItems from '../AssignTagMenuItems';
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

		const { t } = useLocale();

		return (
			<ContextMenu.Item
				label={t('remove_from_recents')}
				onClick={async () => {
					try {
						await removeFromRecents.mutateAsync(
							selectedObjects.map((object) => object.id)
						);
					} catch (error) {
						toast.error({
							title: t('failed_to_remove_file_from_recents'),
							body: t('error_message', { error })
						});
					}
				}}
			/>
		);
	}
});

export const AssignTag = new ConditionalItem({
	useCondition: () => {
		const { selectedItems } = useContextMenuContext();

		const items = selectedItems
			.map((item) => {
				if (item.type === 'Object' || item.type === 'Path') return item;
			})
			.filter(
				(item): item is Extract<ExplorerItem, { type: 'Object' | 'Path' }> =>
					item !== undefined
			);

		if (!isNonEmpty(items)) return null;

		return { items };
	},
	Component: ({ items }) => {
		const { t } = useLocale();
		return (
			<Menu.SubMenu label={t('assign_tag')} icon={TagSimple}>
				<AssignTagMenuItems items={items} />
			</Menu.SubMenu>
		);
	}
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
	Component: ({ kind }) => {
		const { t } = useLocale();
		return (
			<Menu.SubMenu label={t('convert_to')} icon={ArrowBendUpRight}>
				{ObjectConversions[kind]?.map((ext) => (
					<Menu.Item key={ext} label={ext} disabled />
				))}
			</Menu.SubMenu>
		);
	}
});
