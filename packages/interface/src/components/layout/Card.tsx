import clsx from 'clsx';
import { PropsWithChildren } from 'react';

export default function Card(props: PropsWithChildren<{ className?: string }>) {
	return (
		<div
			className={clsx(
				'flex w-full px-4 py-2 border border-gray-500 rounded-lg bg-gray-550',
				props.className
			)}
		>
			{props.children}
		</div>
	);
}
