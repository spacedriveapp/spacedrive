import { getExplorerStore, useExplorerStore } from "~/app/$libraryId/Explorer/store"
import { useLibraryQuery} from '@sd/client';
import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router";
import { useZodRouteParams } from "../hooks/useZodRouteParams";
import { LibraryIdParamsSchema } from "../app/route-schemas";
/**
 * When a user adds a location and checks the should redirect box,
 * this hook will redirect them to the location
 * once the indexer has been invoked
 */

export const useShouldRedirect = () => {
	const { jobsToRedirect } = useExplorerStore();
	const navigate = useNavigate();
	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);
	const jobGroups = useLibraryQuery(['jobs.reports'], {
		enabled: !!(jobsToRedirect.length > 0),
		refetchOnWindowFocus: false,
	});

	//We loop all job groups and pull the first job that matches the location id from the job group

	const pullMatchingJob = useCallback(() => {
		if (jobsToRedirect.length === 0) return;
		let jobFound
			if (jobGroups.data) {
				for (const jobGroup of jobGroups.data) {
					for (const job of jobGroup.jobs) {
						if (job.name === 'indexer') {
							const locationId = jobsToRedirect.find((l) => l.locationId === job.metadata.location.id)?.locationId
							if (job.metadata.location.id === locationId && job.completed_task_count > 0) {
							jobFound = job;
							break;
							}
						}
					}
				}
			}
			return jobFound
	}, [jobGroups.data, jobsToRedirect])

	//Once we have a matching job, we redirect the user to the location

	useEffect(() => {
		if (jobGroups.data) {
			const matchingJob = pullMatchingJob();
			if (matchingJob) {
				const locationId = jobsToRedirect.find((l) => l.locationId === matchingJob.metadata.location.id)?.locationId
				navigate(`/${libraryId}/location/${locationId}`);
				getExplorerStore().jobsToRedirect = jobsToRedirect.filter((l) => l.locationId !== matchingJob.metadata.location.id);
			}
		}
	}, [jobGroups.data, pullMatchingJob, navigate, libraryId, jobsToRedirect])

}


