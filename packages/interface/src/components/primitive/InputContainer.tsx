import clsx from 'clsx';
import { PropsWithChildren } from 'react';
import { DefaultProps } from './types';

interface InputContainerProps extends DefaultProps<HTMLDivElement> {
	title: string;
	description?: string;
	mini?: boolean;
}

export function InputContainer({ mini, ...props }: PropsWithChildren<InputContainerProps>) {
	return (
		<div className="flex flex-row">
			<div {...props} className={clsx('flex flex-col w-full', !mini && 'pb-6', props.className)}>
				<h3 className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">{props.title}</h3>
				{!!props.description && <p className="mb-2 text-sm text-gray-400 ">{props.description}</p>}
				{!mini && props.children}
			</div>
			{mini && props.children}
		</div>
	);
}
