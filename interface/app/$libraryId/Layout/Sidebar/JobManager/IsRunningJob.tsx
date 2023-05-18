import { CheckCircle } from 'phosphor-react';
import { Loader } from '@sd/ui';
import { useLibraryQuery } from '~/../packages/client/src';

export default () => {
	const { data: runningJobs } = useLibraryQuery(['jobs.getRunning']);
	const isRunningJob = runningJobs?.length !== undefined && runningJobs?.length > 0;

	return isRunningJob ? (
		<Loader className="h-[20px] w-[20px]" />
	) : (
		<CheckCircle className="h-5 w-5" />
	);
};
