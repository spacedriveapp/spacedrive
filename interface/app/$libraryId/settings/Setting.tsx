import { Info } from '@phosphor-icons/react';
import clsx from 'clsx';
import { PropsWithChildren, ReactNode } from 'react';
import { ErrorMessage, Tooltip } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

interface Props {
	title: ReactNode;
	registerName?: string;
	description?: string | JSX.Element;
	mini?: boolean;
	className?: string;
	containerClassName?: string;
	toolTipLabel?: string | boolean;
	infoUrl?: string;
}

export default function Setting({ mini, registerName, ...props }: PropsWithChildren<Props>) {
	const platform = usePlatform();

	if (typeof props.description === 'string')
		props.description = <p className="mb-2 text-sm text-gray-400">{props.description}</p>;

	return (
		<>
			<div className={clsx('relative flex flex-row', props.containerClassName)}>
				<div className={clsx('flex w-full flex-col', !mini && 'pb-6', props.className)}>
					<div className="mb-1 flex items-center gap-1">
						<h3 className="font-plex text-sm font-semibold text-ink">{props.title}</h3>
						{props.toolTipLabel && (
							<Tooltip label={props.toolTipLabel as string}>
								<Info
									onClick={() =>
										props.infoUrl && platform.openLink(props.infoUrl)
									}
									size={15}
								/>
							</Tooltip>
						)}
					</div>
					<div className="text-balance">{props.description}</div>
					{!mini && props.children}
				</div>
				{mini && props.children}
			</div>
			{registerName ? (
				<ErrorMessage name={registerName} className="mt-1 w-full text-xs" />
			) : null}
		</>
	);
}
