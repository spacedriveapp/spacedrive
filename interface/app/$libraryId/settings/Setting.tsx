import clsx from 'clsx';
import { Info } from '@phosphor-icons/react';
import { PropsWithChildren } from 'react';
import { Tooltip } from '@sd/ui';

interface Props {
	title: string;
	description?: string;
	mini?: boolean;
	className?: string;
	toolTipLabel?: string | boolean;
}

export default ({ mini, ...props }: PropsWithChildren<Props>) => {
	return (
		<div className="relative flex flex-row">
			<div className={clsx('flex w-full flex-col', !mini && 'pb-6', props.className)}>
				<div className="mb-1 flex items-center gap-1">
					<h3 className="text-sm font-medium text-ink">{props.title}</h3>
					{props.toolTipLabel && (
						<Tooltip label={props.toolTipLabel as string}>
							<Info size={15} />
						</Tooltip>
					)}
				</div>
				{!!props.description && (
					<p className="mb-2 text-sm text-gray-400 ">{props.description}</p>
				)}
				{!mini && props.children}
			</div>
			{mini && props.children}
		</div>
	);
};
