import { getExplorerStore, useExplorerStore } from "~/app/$libraryId/Explorer/store"
import { JobGroup, useLibraryQuery } from '@sd/client';
import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router";
import { useZodRouteParams } from "../hooks/useZodRouteParams";
import { LibraryIdParamsSchema } from "../app/route-schemas";
import { useRef } from "react";

/**
 * When a user adds a location and checks the should redirect box,
 * this hook will redirect them to the location
 * once the indexer has been invoked
 */

export const useShouldRedirect = () => {
	const { shouldRedirectJob } = useExplorerStore();
	const navigate = useNavigate();
	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);
	const jobGroups = useLibraryQuery(['jobs.reports'], {
		enabled: !!shouldRedirectJob.redirect && !!shouldRedirectJob.locationId,
		refetchOnWindowFocus: false,
	});
	const cacheJobGroup = useRef<JobGroup | undefined>(undefined);

	const lookForJob = useCallback(() => {
		let started = false;
		if (!jobGroups.data) return false;
			const newestJobGRoup = jobGroups.data[0]
			if (newestJobGRoup) {
				for (const job of newestJobGRoup.jobs) {
					if (job.name === 'indexer' && job.completed_task_count !== 0) {
						started = true;
						break;
					}
				}
			}
		return started;
	}
	, [jobGroups.data]);

	useEffect(() => {
		cacheJobGroup.current = jobGroups.data?.[0];
	}, [jobGroups.data])

	useEffect(() => {
		if (!shouldRedirectJob.redirect || !shouldRedirectJob.locationId) return;
				const indexerJobFound = lookForJob();
				if (cacheJobGroup.current !== undefined) {
				if (indexerJobFound && shouldRedirectJob.locationId) {
					cacheJobGroup.current = undefined;
					navigate(`/${libraryId}/location/${shouldRedirectJob.locationId}`);
					getExplorerStore().shouldRedirectJob = { redirect: false, locationId: null };
				}
			}
	}, [lookForJob, shouldRedirectJob, navigate, libraryId])
}
