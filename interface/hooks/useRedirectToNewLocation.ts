import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { useLibraryQuery, useSelector } from '@sd/client';
import { explorerStore } from '~/app/$libraryId/Explorer/store';

import { LibraryIdParamsSchema } from '../app/route-schemas';
import { useZodRouteParams } from './useZodRouteParams';

/**
 * When a user adds a location and checks the should redirect box,
 * this hook will redirect them to the location
 * once the indexer has been invoked
 */

export const useRedirectToNewLocation = () => {
	const navigate = useNavigate();
	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);
	const newLocation = useSelector(explorerStore, (s) => s.newLocationToRedirect);
	const { data: jobGroups } = useLibraryQuery(['jobs.reports'], {
		enabled: newLocation != null,
		refetchOnWindowFocus: false
	});

	const hasIndexerJob = jobGroups
		?.flatMap((j) => j.jobs)
		.filter((j) => j.name === 'Indexer')
		.some((j) => {
			for (const metadata of j.metadata) {
				if (metadata.type === 'input' && metadata.metadata.type === 'location') {
					return (
						metadata.metadata.data.id === newLocation &&
						(j.completed_task_count > 0 || j.completed_at != null)
					);
				}
			}

			return false;
		});

	useEffect(() => {
		if (hasIndexerJob) {
			navigate(`/${libraryId}/location/${newLocation}`);
			explorerStore.newLocationToRedirect = null;
		}
	}, [hasIndexerJob, libraryId, newLocation, navigate]);
};
