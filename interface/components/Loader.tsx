import clsx from 'clsx';
import { Puff } from 'react-loading-icons';
import { useIsDark } from '~/hooks';

interface Props {
	className?: string;
	color?: string;
}

export function Loader({ className }: Props) {
	const isDark = useIsDark();
	return (
		<Puff
			stroke={isDark ? '#2599FF' : '#303136'}
			strokeOpacity={4}
			strokeWidth={5}
			speed={1}
			className={clsx('size-7', className)}
		/>
	);
}
