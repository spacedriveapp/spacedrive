import { useLibraryQuery } from '@sd/client';

/*
	This is a hook to check if a location is indexing and completed_task_count is 0.
	We use this to display a loading indicator in the location page.
*/

export const useIsLocationIndexing = (locationId: number): boolean => {
	const { data: jobGroups } = useLibraryQuery(['jobs.reports'], {
		enabled: locationId != null,
		refetchOnWindowFocus: false
	});

	const isLocationIndexing =
		jobGroups?.some((group) =>
			group.jobs.some((job) => {
				if (
					job.name === 'indexer' &&
					job.metadata?.location.id === locationId &&
					(job.status === 'Running' || job.status === 'Queued')
				) {
					return job.completed_task_count === 0;
				}
			})
		) || false;

	return isLocationIndexing;
};
