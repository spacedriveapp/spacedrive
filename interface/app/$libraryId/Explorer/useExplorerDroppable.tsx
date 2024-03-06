import { useDroppable, UseDroppableArguments } from '@dnd-kit/core';
import { useEffect, useId, useMemo, useState } from 'react';
import { NavigateOptions, To, useNavigate } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import { z } from 'zod';
import { ExplorerItem, getItemFilePath, Location, Tag } from '@sd/client';

import { useExplorerContext } from './Context';
import { explorerStore } from './store';

type ExplorerItemType = ExplorerItem['type'];

const droppableTypes = [
	'Location',
	'NonIndexedPath',
	'Object',
	'Path',
	'SpacedropPeer'
] satisfies ExplorerItemType[];

export interface UseExplorerDroppableProps extends Omit<UseDroppableArguments, 'id'> {
	id?: string;
	data?:
		| {
				type: 'location';
				data?: Location | z.infer<typeof explorerLocationSchema>['data'];
				path: string;
		  }
		| { type: 'explorer-item'; data: ExplorerItem }
		| { type: 'tag'; data: Tag };
	allow?: ExplorerItemType | ExplorerItemType[];
	navigateTo?: To | { to: To; options?: NavigateOptions } | number | (() => void);
}

const explorerPathSchema = z.object({
	type: z.literal('Path'),
	item: z.object({
		id: z.number(),
		name: z.string(),
		location_id: z.number(),
		materialized_path: z.string()
	})
});

const explorerObjectSchema = z.object({
	type: z.literal('Object'),
	item: z.object({
		file_paths: explorerPathSchema.shape.item.array()
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

const explorerItemSchema = z.object({
	type: z.literal('explorer-item'),
	data: explorerPathSchema
		.or(explorerNonIndexedPathSchema)
		.or(explorerItemLocationSchema)
		.or(explorerObjectSchema)
});

const explorerLocationSchema = z.object({
	type: z.literal('location'),
	data: z.object({ id: z.number(), path: z.string() }).optional(),
	path: z.string()
});

const explorerTagSchema = z.object({
	type: z.literal('tag'),
	data: z.object({ id: z.number() })
});

export const explorerDroppableSchema = explorerItemSchema
	.or(explorerLocationSchema)
	.or(explorerTagSchema);

export const useExplorerDroppable = ({
	allow,
	navigateTo,
	...props
}: UseExplorerDroppableProps) => {
	const id = useId();
	const navigate = useNavigate();

	const explorer = useExplorerContext({ suspense: false });

	const [canNavigate, setCanNavigate] = useState(true);

	const { setNodeRef, ...droppable } = useDroppable({
		...props,
		id: props.id ?? id,
		disabled: (!props.data && !navigateTo) || props.disabled
	});

	const isDroppable = useMemo(() => {
		if (!droppable.isOver) return false;

		const drag = explorerStore.drag; // TODO: This should probs be a snapshot but it was like this prior to this PR.
		if (!drag || drag.type === 'touched') return false;

		let allowedType: ExplorerItemType | ExplorerItemType[] | undefined = allow;

		if (!allowedType) {
			if (explorer?.parent) {
				switch (explorer.parent.type) {
					case 'Location':
					case 'Ephemeral': {
						allowedType = ['Path', 'NonIndexedPath', 'Object'];
						break;
					}
					case 'Tag': {
						allowedType = ['Path', 'Object'];
						break;
					}
				}
			} else if (props.data?.type === 'explorer-item') {
				switch (props.data.data.type) {
					case 'Path':
					case 'NonIndexedPath': {
						allowedType = ['Path', 'NonIndexedPath', 'Object'];
						break;
					}

					case 'Object': {
						allowedType = ['Path', 'Object'];
						break;
					}
				}
			} else allowedType = droppableTypes;

			if (!allowedType) return false;
		}

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

		return schema.safeParse(drag.items[0]).success;
	}, [allow, droppable.isOver, explorer?.parent, props.data]);

	const filePath = props.data?.type === 'explorer-item' && getItemFilePath(props.data.data);
	const isLocation = props.data?.type === 'explorer-item' && props.data.data.type === 'Location';

	const isNavigable = isDroppable && canNavigate && (filePath || navigateTo || isLocation);

	useEffect(() => {
		if (!isNavigable) return;

		const timeout = setTimeout(() => {
			if (navigateTo) {
				if (typeof navigateTo === 'function') {
					navigateTo();
				} else if (typeof navigateTo === 'object' && 'to' in navigateTo) {
					navigate(navigateTo.to, navigateTo.options);
				} else if (typeof navigateTo === 'number') {
					navigate(navigateTo);
				} else {
					navigate(navigateTo);
				}
			} else if (filePath) {
				if ('id' in filePath) {
					navigate({
						pathname: `../location/${filePath.location_id}`,
						search: `${createSearchParams({
							path: `${filePath.materialized_path}${filePath.name}/`
						})}`
					});
				} else {
					navigate({ search: `${createSearchParams({ path: filePath.path })}` });
				}
			} else if (
				props.data?.type === 'explorer-item' &&
				props.data.data.type === 'Location'
			) {
				navigate(`../location/${props.data.data.item.id}`);
			}

			// Timeout navigation
			setCanNavigate(false);
			setTimeout(() => setCanNavigate(true), 1250);
		}, 1250);

		return () => clearTimeout(timeout);
	}, [navigate, props.data, navigateTo, filePath, isNavigable]);

	const className = isNavigable
		? 'animate-pulse transition-opacity duration-200 [animation-delay:1000ms]'
		: undefined;

	return { setDroppableRef: setNodeRef, ...droppable, isDroppable, className };
};
