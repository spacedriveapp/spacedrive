import { useDroppable, UseDroppableArguments } from '@dnd-kit/core';
import { useEffect, useId, useMemo, useState } from 'react';
import { NavigateOptions, To, useNavigate } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import { z } from 'zod';
import { ExplorerItem, getItemFilePath, Location } from '@sd/client';

import { getExplorerStore } from '../store';
import { uniqueId } from '../util';

type ExplorerItemType = ExplorerItem['type'];

const types = [
	'Location',
	'NonIndexedPath',
	'Object',
	'Path',
	'SpacedropPeer'
] satisfies ExplorerItemType[];

interface Props extends Omit<UseDroppableArguments, 'id'> {
	id?: string;
	data?:
		| { type: 'location'; data: Location; path: string }
		| { type: 'explorer-item'; data: ExplorerItem };
	allow?: ExplorerItemType | ExplorerItemType[];
	navigateTo?: To | { to: To; options?: NavigateOptions } | number | (() => void);
}

export const explorerDroppableLocationSchema = z.object({
	type: z.literal('location'),
	path: z.string(),
	data: z.object({ id: z.number() })
});

export const explorerDroppableItemSchema = z.object({
	type: z.literal('explorer-item'),
	data: z.object({
		type: z.literal('Path'),
		item: z.object({
			id: z.number(),
			name: z.string(),
			location_id: z.number(),
			materialized_path: z.string()
		})
	})
});

export const explorerDroppableSchema = explorerDroppableLocationSchema.or(
	explorerDroppableItemSchema
);

export const useExplorerDroppable = ({ allow, navigateTo, ...props }: Props) => {
	const navigate = useNavigate();
	const id = useId();

	const [canNavigate, setCanNavigate] = useState(true);

	const { setNodeRef, ...droppable } = useDroppable({
		...props,
		id:
			props.id ??
			(props.data
				? 'type' in props.data.data
					? uniqueId(props.data.data)
					: props.data.data.id
				: id),
		disabled: (!props.data && !navigateTo) || props.disabled
	});

	const resetNavigate = () => {
		setCanNavigate(false);
		setTimeout(() => setCanNavigate(true), 2000);
	};

	const blocked = useMemo(() => {
		if (!droppable.isOver) return true;

		const { drag } = getExplorerStore();
		if (!drag || drag.type === 'touched') return true;

		const allowed = !allow ? types : Array.isArray(allow) ? allow : [allow];

		const schema = z.object({
			type: z.union(
				allowed.map((type) => z.literal(type)) as unknown as [
					z.ZodLiteral<ExplorerItemType>,
					z.ZodLiteral<ExplorerItemType>,
					...z.ZodLiteral<ExplorerItemType>[]
				]
			)
		});

		return !schema.safeParse(drag.items[0]).success;
	}, [allow, droppable.isOver]);

	const isDroppable = droppable.isOver && !blocked;

	const filePathData = useMemo(() => {
		if (!isDroppable || !props.data || !('type' in props.data.data)) return;
		return getItemFilePath(props.data.data);
	}, [isDroppable, props.data]);

	useEffect(() => {
		if (!isDroppable || !canNavigate || (!filePathData && !navigateTo)) return;

		const timeout = setTimeout(() => {
			if (navigateTo) {
				if (typeof navigateTo === 'function') navigateTo();
				else if (typeof navigateTo === 'object' && 'to' in navigateTo) {
					navigate(navigateTo.to, navigateTo.options);
				} else if (typeof navigateTo === 'number') {
					navigate(navigateTo);
				} else {
					navigate(navigateTo);
				}
			} else if (filePathData) {
				if ('id' in filePathData) {
					navigate({
						pathname: `../location/${filePathData.location_id}`,
						search: createSearchParams({
							path: `${filePathData.materialized_path}${filePathData.name}/`
						}).toString()
					});
				} else {
					navigate({
						search: createSearchParams({ path: filePathData.path }).toString()
					});
				}
			}

			resetNavigate();
		}, 2000);

		return () => clearTimeout(timeout);
	}, [isDroppable, navigate, props.data, navigateTo, filePathData, canNavigate]);

	const navigateClassName =
		isDroppable && canNavigate && (filePathData || navigateTo)
			? 'animate-pulse duration-200 [animation-delay:1700ms]'
			: undefined;

	return { setDroppableRef: setNodeRef, ...droppable, isDroppable, navigateClassName };
};
