import { memo } from 'react';
import { JobReport } from '@sd/client';
import GroupedJob from './GroupedJob';

interface Props {
	clearJob: (arg: string) => void;
	runningJobs?: JobReport[];
	jobs?: JobReport[];
}

export interface IGroupedJobs extends JobReport {
	childJobs: JobReport[];
	runningJobs: JobReport[];
}

function GroupedJobs({ clearJob, jobs, runningJobs }: Props) {
	const groupJobsByParentId = () => {
		const arr = [];
		if (jobs) {
			for (const job of jobs) {
				const childJobs = jobs.filter((j) => j.parent_id === job.id);
				const parentJob = jobs.find((j) => j.id === job.parent_id) || null;
				if (parentJob === null) {
					arr.push({
						...job,
						childJobs,
						runningJobs: runningJobs?.filter((j) => j.parent_id === job.id)
					});
				}
			}
		}
		return arr as IGroupedJobs[];
	};

	return (
		<>
			{groupJobsByParentId().map((data) => {
				return (
					<div key={data.id}>
						<GroupedJob data={data} clearJob={clearJob} />
					</div>
				);
			})}
		</>
	);
}

export default memo(GroupedJobs);
