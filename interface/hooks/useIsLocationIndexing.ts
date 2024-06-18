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
				let jobLocationId: number | undefined;
				for (const metadata of job.metadata) {
					if (metadata.type === 'input' && metadata.metadata.type === 'location') {
						jobLocationId = metadata.metadata.data.id;
						break;
					}
				}
				if (
					job.name === 'Indexer' &&
					jobLocationId === locationId &&
					(job.status === 'Running' || job.status === 'Queued')
				) {
					return job.completed_task_count === 0;
				}
				return false;
			})
		) || false;

	return isLocationIndexing;
};
