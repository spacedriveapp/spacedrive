import clsx from 'clsx';
import { Puff } from 'react-loading-icons';

export default function Loader(props: { className?: string }) {
	return (
		<Puff
			stroke="#2599FF"
			strokeOpacity={4}
			strokeWidth={5}
			speed={1}
			className={clsx('ml-0.5 mt-[2px] -mr-1 w-7 h-7', props.className)}
		/>
	);
}
