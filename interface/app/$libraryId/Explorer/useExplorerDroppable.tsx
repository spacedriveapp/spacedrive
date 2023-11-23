import { useDroppable, UseDroppableArguments } from '@dnd-kit/core';
import { CSSProperties, useEffect, useId, useMemo, useState } from 'react';
import { NavigateOptions, To, useNavigate } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import { z } from 'zod';
import { ExplorerItem, getItemFilePath, Location } from '@sd/client';

import { useExplorerContext } from './Context';
import { getExplorerStore } from './store';
import { uniqueId } from './util';

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
		| { type: 'location'; data?: Location; path: string }
		| { type: 'explorer-item'; data: ExplorerItem };
	allow?: ExplorerItemType | ExplorerItemType[];
	navigateTo?: To | { to: To; options?: NavigateOptions } | number | (() => void);
}

const explorerLocationSchema = z.object({
	type: z.literal('location'),
	data: z.object({ id: z.number(), path: z.string() }).optional(),
	path: z.string()
});

const explorerPathSchema = z.object({
	type: z.literal('Path'),
	item: z.object({
		id: z.number(),
		name: z.string(),
		location_id: z.number(),
		materialized_path: z.string()
	})
});

const explorerNonIndexedPathSchema = z.object({
	type: z.literal('NonIndexedPath'),
	item: z.object({
		name: z.string(),
		path: z.string()
	})
});

const explorerItemLocationSchema = z.object({
	type: z.literal('Location'),
	item: z.object({ id: z.number(), path: z.string() })
});

export const explorerDroppableItemSchema = z.object({
	type: z.literal('explorer-item'),
	data: explorerPathSchema.or(explorerNonIndexedPathSchema).or(explorerItemLocationSchema)
});

export const explorerDroppableSchema = explorerLocationSchema.or(explorerDroppableItemSchema);

export const useExplorerDroppable = ({ allow, navigateTo, ...props }: Props) => {
	const id = useId();
	const navigate = useNavigate();

	const explorer = useExplorerContext({ suspense: false });

	const [canNavigate, setCanNavigate] = useState(true);

	const { setNodeRef, ...droppable } = useDroppable({
		...props,
		id:
			props.id ??
			(props.data
				? props.data.type !== 'location'
					? uniqueId(props.data.data)
					: props.data.data?.id ?? props.data.path
				: id),
		disabled: (!props.data && !navigateTo) || props.disabled
	});

	const resetNavigate = () => {
		setCanNavigate(false);
		setTimeout(() => setCanNavigate(true), 1250);
	};

	const blocked = useMemo(() => {
		if (!droppable.isOver) return true;

		const { drag } = getExplorerStore();
		if (!drag || drag.type === 'touched') return true;

		let allowedType: ExplorerItemType | ExplorerItemType[] | undefined = allow;

		if (!allowedType) {
			if (!explorer?.parent) allowedType = types;
			else {
				switch (explorer.parent.type) {
					case 'Location': {
						allowedType = ['Path', 'NonIndexedPath'];
						break;
					}

					case 'Ephemeral': {
						allowedType = ['Path', 'NonIndexedPath'];
						break;
					}

					case 'Tag': {
						allowedType = 'Object';
						break;
					}
				}
			}
		}

		if (!allowedType) return true;

		const schema = z.object({
			type: Array.isArray(allowedType)
				? z.union(
						allowedType.map((type) => z.literal(type)) as unknown as [
							z.ZodLiteral<ExplorerItemType>,
							z.ZodLiteral<ExplorerItemType>,
							...z.ZodLiteral<ExplorerItemType>[]
						]
				  )
				: z.literal(allowedType)
		});

		return !schema.safeParse(drag.items[0]).success;
	}, [allow, droppable.isOver, explorer?.parent]);

	const isDroppable = droppable.isOver && !blocked;

	const filePathData = useMemo(() => {
		if (!isDroppable || !props.data || props.data.type === 'location') return;
		return getItemFilePath(props.data.data);
	}, [isDroppable, props.data]);

	useEffect(() => {
		if (!isDroppable || !canNavigate || (!props.data && !navigateTo)) return;

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

			if (props.data?.type === 'explorer-item') {
				if (props.data.data.type === 'Location') {
					console.log(props.data.data);
					navigate(`../location/${props.data.data.item.id}`);
				}
			}

			resetNavigate();
		}, 1250);

		return () => clearTimeout(timeout);
	}, [isDroppable, navigate, props.data, navigateTo, filePathData, canNavigate]);

	const navigateClassName =
		isDroppable &&
		canNavigate &&
		(filePathData ||
			navigateTo ||
			(props.data?.type === 'explorer-item' && props.data.data.type === 'Location'))
			? 'animate-pulse transition-opacity duration-200 [animation-delay:1000ms]'
			: undefined;

	const style = {
		cursor: droppable.isOver && blocked ? 'no-drop' : undefined
	} satisfies CSSProperties;

	return { setDroppableRef: setNodeRef, ...droppable, isDroppable, navigateClassName, style };
};
