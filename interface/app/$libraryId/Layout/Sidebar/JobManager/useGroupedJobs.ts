import { useEffect, useState } from 'react';
import { JobReport } from '@sd/client';

export interface IJobGroup {
	queued?: boolean;
	id: string;
	action: string;
	completed?: boolean;
	jobs: JobReport[];
}

export function useGroupedJobs(jobs: JobReport[] = []): IJobGroup[] {
	const [groups, setGroups] = useState<IJobGroup[]>([]);

	useEffect(() => {
		const groupsObj: { [key: string]: IJobGroup } = {};

		for (const job of jobs) {
			// Skip job if action is null
			if (!job.action) continue;

			// Split the action identifier at the hyphen and take the first part as the action name.
			const actionName = job.action.split('-')[0];

			// Define a key for the group. For parent jobs, the key is actionName-id. For child jobs, it is actionName-parent_id.
			const key = job.parent_id
				? `${actionName}-${job.parent_id}`
				: `${actionName}-${job.id}`;

			// Check if the group key already exists, if not initialize one.
			if (actionName && !groupsObj[key]) {
				groupsObj[key] = {
					id: job.parent_id || job.id, // Use parent_id for child jobs and id for parent jobs.
					action: actionName,
					jobs: []
				};
			}

			// Add the current job to its group.
			groupsObj[key]?.jobs.unshift(job);
		}

		// Convert the groups object to an array of JobGroup objects.
		const groupsArray: IJobGroup[] = Object.values(groupsObj);

		groupsArray.map((group) => {
			// Check if all jobs in the group are completed.
			const completed = group.jobs.every((job) => job.status === 'Completed');
			const queued = group.jobs.every((job) => job.status === 'Queued');

			// Add the completed property to the group.
			group.completed = completed;
			group.queued = queued;
		});

		// Update the state.
		setGroups(groupsArray);
	}, [jobs]); // Only rerun the effect if the jobs array changes.

	return groups;
}
