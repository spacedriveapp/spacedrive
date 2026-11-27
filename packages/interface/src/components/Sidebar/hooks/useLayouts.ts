import {useNormalizedQuery} from '@sd/ts-client';
import type {
	Event,
	SidebarLayout,
	SidebarLayoutQueryInput,
	LayoutsListOutput,
	LayoutsListQueryInput
} from '@sd/ts-client';
import {useQueryClient} from '@tanstack/react-query';
import {useEffect} from 'react';
import {useSpacedriveClient} from '../../../contexts/SpacedriveContext';

export function useLayouts() {
	return useNormalizedQuery<LayoutsListQueryInput, LayoutsListOutput>({
		query: 'layouts.list',
		input: null, // Unit struct serializes as null, not {}
		resourceType: 'layout'
	});
}

export function useSidebarLayout(layoutId: string | null) {
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();
	const libraryId = client.getCurrentLibraryId();

	const query = useNormalizedQuery<SidebarLayoutQueryInput | null, SidebarLayout>(
		{
			query: 'layouts.get_sidebar',
			input: layoutId ? {layout_id: layoutId} : null,
			resourceType: 'sidebar_layout',
			resourceId: layoutId || undefined,
			enabled: !!layoutId
		}
	);

	// Subscribe to layout_item deletions to update the layout
	// (layout_item sends its own ResourceDeleted events, separate from sidebar_layout)
	useEffect(() => {
		if (!layoutId || !libraryId) return;

		const handleEvent = (event: Event) => {
			if (typeof event === 'string') return;

			if ('ResourceDeleted' in event) {
				const {resource_type, resource_id} = event.ResourceDeleted;

				if (resource_type === 'layout_item') {
					console.log(
						'[useSidebarLayout] Layout item deleted, updating layout:',
						resource_id
					);

					// Remove the item from the layout cache
					const queryKey = [
						'query:layouts.get_sidebar',
						libraryId,
						{layout_id: layoutId}
					];
					queryClient.setQueryData<SidebarLayout>(
						queryKey,
						(oldData) => {
							if (!oldData) return oldData;

							const updatedLayoutItems =
								oldData.layout_items.filter(
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
								layout_items: updatedLayoutItems,
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
					resource_type: 'layout_item',
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
	}, [client, queryClient, layoutId, libraryId]);

	return query;
}
