import {useNormalizedQuery} from '@sd/ts-client';
import type {
	Event,
	SpaceLayout,
	SpaceLayoutQueryInput,
	SpacesListOutput,
	SpacesListQueryInput
} from '@sd/ts-client';
import {useQueryClient} from '@tanstack/react-query';
import {useEffect} from 'react';
import {useSpacedriveClient} from '../../../contexts/SpacedriveContext';

export function useSpaces() {
	return useNormalizedQuery<SpacesListQueryInput, SpacesListOutput>({
		query: 'spaces.list',
		input: null, // Unit struct serializes as null, not {}
		resourceType: 'space'
	});
}

export function useSpaceLayout(spaceId: string | null) {
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();
	const libraryId = client.getCurrentLibraryId();

	const query = useNormalizedQuery<SpaceLayoutQueryInput | null, SpaceLayout>(
		{
			query: 'spaces.get_layout',
			input: spaceId ? {space_id: spaceId} : null,
			resourceType: 'space_layout',
			resourceId: spaceId || undefined,
			enabled: !!spaceId
		}
	);

	// Subscribe to space_item deletions to update the layout
	// (space_item sends its own ResourceDeleted events, separate from space_layout)
	useEffect(() => {
		if (!spaceId || !libraryId) return;

		const handleEvent = (event: Event) => {
			if (typeof event === 'string') return;

			if ('ResourceDeleted' in event) {
				const {resource_type, resource_id} = event.ResourceDeleted;

				if (resource_type === 'space_item') {
					console.log(
						'[useSpaceLayout] Space item deleted, updating layout:',
						resource_id
					);

					// Remove the item from the layout cache
					const queryKey = [
						'query:spaces.get_layout',
						libraryId,
						{space_id: spaceId}
					];
					queryClient.setQueryData<SpaceLayout>(
						queryKey,
						(oldData) => {
							if (!oldData) return oldData;

							const updatedSpaceItems =
								oldData.space_items.filter(
									(item) => item.id !== resource_id
								);

							const updatedGroups = oldData.groups.map(
								(group) => ({
									...group,
									items: group.items.filter(
										(item) => item.id !== resource_id
									)
								})
							);

							return {
								...oldData,
								space_items: updatedSpaceItems,
								groups: updatedGroups
							};
						}
					);
				}
			}
		};

		let unsubscribe: (() => void) | undefined;

		client
			.subscribeFiltered(
				{
					resource_type: 'space_item',
					library_id: libraryId,
					include_descendants: false
				},
				handleEvent
			)
			.then((unsub) => {
				unsubscribe = unsub;
			});

		return () => {
			unsubscribe?.();
		};
	}, [client, queryClient, spaceId, libraryId]);

	return query;
}
