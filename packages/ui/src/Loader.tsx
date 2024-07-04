import clsx from 'clsx';
import { Puff } from 'react-loading-icons';

export function Loader(props: { className?: string; color?: string }) {
	return (
		<Puff
			stroke={props.color || '#2599FF'}
			strokeOpacity={4}
			strokeWidth={5}
			speed={1}
			className={clsx('size-7', props.className)}
		/>
	);
}
