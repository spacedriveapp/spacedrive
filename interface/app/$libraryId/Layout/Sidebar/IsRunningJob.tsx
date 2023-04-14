import { CheckCircle } from 'phosphor-react';
import { Loader } from '@sd/ui';
import { useLibraryQuery } from '~/../packages/client/src';

export default () => {
	const { data: isRunningJob } = useLibraryQuery(['jobs.isRunning']);

	return isRunningJob ? (
		<Loader className="h-[20px] w-[20px]" />
	) : (
		<CheckCircle className="h-5 w-5" />
	);
};
