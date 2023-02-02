import clsx from 'clsx';
import { Puff } from 'react-loading-icons';

export function Loader(props: { className?: string }) {
	return (
		<Puff
			stroke="#2599FF"
			strokeOpacity={4}
			strokeWidth={5}
			speed={1}
			className={clsx('h-7 w-7', props.className)}
		/>
	);
}
