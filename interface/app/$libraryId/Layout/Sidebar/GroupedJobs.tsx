import { memo } from 'react';
import { JobReport } from '@sd/client';
import GroupedJob from './GroupedJob';

interface Props {
	clearAJob: (arg: string) => void;
	runningJobs?: JobReport[];
	jobs?: JobReport[];
}

function GroupedJobs({ clearAJob, jobs, runningJobs }: Props) {
	const groupJobsByParentId = () => {
		const data: Record<string, JobReport[]> =
			jobs?.reduce((acc, job) => {
				if (job.parent_id) {
					acc[job.parent_id] = acc[job.parent_id] || [];
					acc[job.parent_id]?.push(job);
				} else {
					acc[job.id] = acc[job.id] || [];
					acc[job.id]?.push(job);
				}
				return acc;
			}, {} as Record<string, JobReport[]>) || {};
		return data;
	};

	const dataStructureOfJobs = Object.entries(groupJobsByParentId()).map(
		([parentId, jobsGroup]) => {
			const parentJob = jobs?.find((job) => job.id === parentId);
			const runningJobsGroup = runningJobs?.filter((job) => job.id === parentId);
			return [{ jobs: jobsGroup, parentJob, runningJobs: runningJobsGroup }];
		}
	);

	return (
		<>
			{dataStructureOfJobs.map((data) => {
				return (
					<div key={data[0]?.parentJob?.id}>
						<GroupedJob
							parentJob={data[0]?.parentJob}
							runningJobs={data[0]?.runningJobs}
							clearAJob={clearAJob}
							jobs={data[0]?.jobs}
						/>
					</div>
				);
			})}
		</>
	);
}

export default memo(GroupedJobs);
