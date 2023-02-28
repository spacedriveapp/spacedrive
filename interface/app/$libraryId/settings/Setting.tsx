import clsx from 'clsx';
import { PropsWithChildren } from 'react';

interface Props {
	title: string;
	description?: string;
	mini?: boolean;
	className?: string;
}

export default ({ mini, ...props }: PropsWithChildren<Props>) => {
	return (
		<div className="flex flex-row">
			<div className={clsx('flex w-full flex-col', !mini && 'pb-6', props.className)}>
				<h3 className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">{props.title}</h3>
				{!!props.description && <p className="mb-2 text-sm text-gray-400 ">{props.description}</p>}
				{!mini && props.children}
			</div>
			{mini && props.children}
		</div>
	);
};
