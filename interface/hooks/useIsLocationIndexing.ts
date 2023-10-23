import { useLibraryQuery } from '@sd/client';

/*
	This is a hook to check if a location is indexing or not.
	We use this to display a loading indicator in the location page.
*/

export const useIsLocationIndexing = (locationId: number): boolean => {

	const { data: jobGroups } = useLibraryQuery(['jobs.reports'], {
		enabled: locationId != null,
		refetchOnWindowFocus: false
	});

	const isLocationIndexing = jobGroups?.some(group =>
		group.jobs.some(job =>
			job.name === 'indexer' && job.metadata.location.id === locationId && job.status !== 'Completed'
		)
	) || false;

	return isLocationIndexing;
}

