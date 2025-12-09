import { useNormalizedQuery } from '@sd/ts-client';
import { useSpacedriveClient } from '../../../context';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import type { Event } from '@sd/ts-client';

export function useSpaces() {
	return useNormalizedQuery({
		wireMethod: 'query:spaces.list',
		input: null, // Unit struct serializes as null, not {}
		resourceType: 'space',
	});
}

export function useSpaceLayout(spaceId: string | null) {
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();
	const libraryId = client.getCurrentLibraryId();

	const query = useNormalizedQuery({
		wireMethod: 'query:spaces.get_layout',
		input: spaceId ? { space_id: spaceId } : null,
		resourceType: 'space_layout',
		resourceId: spaceId || undefined,
		enabled: !!spaceId,
	});

	// Subscribe to space_item deletions to update the layout
	// (space_item sends its own ResourceDeleted events, separate from space_layout)
	useEffect(() => {
		if (!spaceId || !libraryId) return;

		const handleEvent = (event: Event) => {
			if (typeof event === 'string') return;

			if ('ResourceDeleted' in event) {
				const { resource_type, resource_id } = (event as any).ResourceDeleted;

				if (resource_type === 'space_item') {
					console.log('[useSpaceLayout] Space item deleted, updating layout:', resource_id);

					// Remove the item from the layout cache
					const queryKey = ['query:spaces.get_layout', libraryId, { space_id: spaceId }];
					queryClient.setQueryData(queryKey, (oldData: any) => {
						if (!oldData) return oldData;

						// Remove from space_items array
						const updatedSpaceItems = oldData.space_items?.filter(
							(item: any) => item.id !== resource_id
						) || [];

						// Remove from groups
						const updatedGroups = oldData.groups?.map((group: any) => ({
							...group,
							items: group.items.filter((item: any) => item.id !== resource_id),
						})) || [];

						return {
							...oldData,
							space_items: updatedSpaceItems,
							groups: updatedGroups,
						};
					});
				}
			}
		};

		let unsubscribe: (() => void) | undefined;

		client.subscribeFiltered(
			{
				resource_type: 'space_item',
				library_id: libraryId,
				include_descendants: false,
			},
			handleEvent
		).then((unsub) => {
			unsubscribe = unsub;
		});

		return () => {
			unsubscribe?.();
		};
	}, [client, queryClient, spaceId, libraryId]);

	return query;
}
