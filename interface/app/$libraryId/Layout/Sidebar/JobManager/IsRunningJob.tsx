import { CheckCircle } from '@phosphor-icons/react';
import { Loader } from '@sd/ui';
import { useLibraryQuery } from '~/../packages/client/src';

export default () => {
	const { data: isActive } = useLibraryQuery(['jobs.isActive']);
	return isActive ? <Loader className="size-[20px]" /> : <CheckCircle className="size-5" />;
};
