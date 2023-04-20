import { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { getExplorerStore } from '~/hooks/useExplorerStore';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import Explorer from '../Explorer';

export function useExplorerParams() {
	const { id } = useParams<{ id?: string }>();
	const location_id = id ? Number(id) : null;

	const [searchParams] = useSearchParams();
	const path = searchParams.get('path') || '';
	const limit = Number(searchParams.get('limit')) || 100;

	return { location_id, path, limit };
}

export const Component = () => {
	const { location_id, path, limit } = useExplorerParams();

	// we destructure this since `mutate` is a stable reference but the object it's in is not
	const { mutate: mutateQuickRescan, ...quickRescan } =
		useLibraryMutation('locations.quickRescan');

	const explorerStore = useExplorerStore();
	const explorerState = getExplorerStore();

	useEffect(() => {
		explorerState.locationId = location_id;
		if (location_id !== null) mutateQuickRescan({ location_id, sub_path: path });
	}, [explorerState, location_id, path, mutateQuickRescan]);

	if (location_id === null) throw new Error(`location_id is null!`);

	const explorerData = useLibraryQuery([
		'locations.getExplorerData',
		{
			location_id,
			path: explorerStore.layoutMode === 'media' ? null : path,
			limit,
			cursor: null,
			kind: explorerStore.layoutMode === 'media' ? [5, 7] : null
		}
	]);

	return (
		<div className="relative flex w-full flex-col">
			<Explorer data={explorerData.data} />
		</div>
	);
};
