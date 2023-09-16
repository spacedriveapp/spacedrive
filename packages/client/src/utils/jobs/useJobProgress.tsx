import { useEffect, useState } from 'react';

import { JobGroup, JobProgressEvent } from '../../core';
import { useLibrarySubscription } from '../../rspc';
import { useJobManagerContext } from './context';

export const useJobProgress = (jobGroups?: JobGroup[]) => {
	const ctx = useJobManagerContext();

	// Create initial progress from cached progress
	const [progress, setProgress] = useState<Record<string, JobProgressEvent>>(() => {
		return {
			...ctx.cachedJobProgress.current
		};
	});

	useLibrarySubscription(['jobs.progress'], {
		onData(data) {
			// console.log(`setting ${data.id} progress`);
			setProgress((prev) => ({ ...prev, [data.id]: data }));
		}
	});

	// Update cached progress when progress changes
	useEffect(() => {
		ctx.cachedJobProgress.current = progress;
	}, [progress, ctx.cachedJobProgress]);

	// Remove jobs that aren't running from progress
	// This can happen kind of lazily since it's not a huge deal
	useEffect(() => {
		if (!jobGroups) return;

		setProgress((prev) => {
			const ret: typeof prev = {};

			for (const group of jobGroups) {
				for (const job of group.jobs) {
					const prevEvent = prev[job.id];
					if (job.status !== 'Running' || !prevEvent) continue;

					ret[job.id] = prevEvent;
				}
			}

			return ret;
		});
	}, [jobGroups]);

	return progress;
};
