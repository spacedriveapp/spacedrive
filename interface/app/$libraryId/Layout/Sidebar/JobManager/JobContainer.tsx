import clsx from 'clsx';
import {
	forwardRef,
	ForwardRefExoticComponent,
	Fragment,
	HTMLAttributes,
	ReactNode,
	useEffect,
	useState
} from 'react';
import { TextItems } from '@sd/client';
import { Tooltip, tw } from '@sd/ui';

import classes from './Job.module.scss';

interface JobContainerProps extends HTMLAttributes<HTMLLIElement> {
	name: string;
	icon?: string | ForwardRefExoticComponent<any>;
	textItems?: TextItems;
	isChild?: boolean;
	children?: ReactNode;
	eta?: number;
	status?: string;
}

const CIRCLE_ICON_CLASS =
	'relative flex-shrink-0 top-1 z-20 mr-3 h-7 w-7 rounded-full bg-app-button p-[5.5px]';
const IMG_ICON_CLASS = 'relative left-[-2px] top-1 z-10 mr-2 h-8 w-8';

const MetaContainer = tw.div`flex w-full overflow-hidden flex-col`;
const TextLine = tw.div`mt-[2px] gap-1 text-ink-faint truncate mr-8 pl-1.5`;
const TextItem = tw.span`truncate`;

const formatETA = (eta: number): string => {
	const seconds = Math.floor((eta / 1000) % 60);
	const minutes = Math.floor((eta / (1000 * 60)) % 60);
	const hours = Math.floor((eta / (1000 * 60 * 60)) % 24);
	const days = Math.floor(eta / (1000 * 60 * 60 * 24));

	let formattedETA = '';

	if (days > 0) formattedETA += `${days} day${days > 1 ? 's' : ''} `;
	if (hours > 0) formattedETA += `${hours} hour${hours > 1 ? 's' : ''} `;
	if (minutes > 0) formattedETA += `${minutes} minute${minutes > 1 ? 's' : ''} `;
	if (seconds > 0 || formattedETA === '')
		formattedETA += `${seconds} second${seconds != 1 ? 's' : ''} `;

	return formattedETA.trim() + ' remaining';
};

// Job container consolidates the common layout of a job item, used for regular jobs (Job.tsx) and grouped jobs (JobGroup.tsx).
const JobContainer = forwardRef<HTMLLIElement, JobContainerProps>((props, ref) => {
	const {
		name,
		icon: Icon,
		textItems,
		isChild,
		children,
		className,
		eta,
		status,
		...restProps
	} = props;
	const [currentETA, setCurrentETA] = useState<number | undefined>(eta);

	useEffect(() => {
		if (currentETA != null && currentETA > 0) {
			const interval = setInterval(() => {
				setCurrentETA((prevETA) => {
					if (prevETA === undefined || prevETA <= 1000) return 0;
					return prevETA - 1000;
				});
			}, 1000);

			return () => clearInterval(interval);
		}
	}, [currentETA]);

	useEffect(() => {
		setCurrentETA(eta);
	}, [eta]);

	return (
		<li
			ref={ref}
			className={clsx(
				'relative flex border-b border-app-line/50 px-4 py-3',
				isChild && classes.jobGroupChild,
				isChild && 'border-none bg-app-darkBox p-2 pl-10',
				className
			)}
			{...restProps}
		>
			{typeof Icon === 'string' ? (
				<img src={Icon} className={IMG_ICON_CLASS} />
			) : (
				Icon && (
					<Icon weight="fill" className={clsx(CIRCLE_ICON_CLASS, isChild && 'mx-1')} />
				)
			)}
			<MetaContainer>
				<Tooltip
					labelClassName="break-all"
					asChild
					tooltipClassName="max-w-[400px]"
					position="top"
					label={name}
				>
					<p className="w-fit max-w-[83%] truncate pl-1.5 font-plex font-semibold tracking-normal">
						{name}
					</p>
				</Tooltip>
				{textItems?.map((item, index) => {
					const filteredItems = item.filter((i) => i?.text);

					const popoverText = filteredItems.map((i) => i?.text).join(' • ');

					return (
						<Tooltip
							label={popoverText}
							key={index}
							tooltipClassName="max-w-[400px] tabular-nums"
						>
							<TextLine>
								{filteredItems.map((textItem, index) => {
									const Icon = textItem?.icon;
									return (
										<Fragment key={index}>
											<TextItem
												onClick={textItem?.onClick}
												className={clsx(
													'tabular-nums',
													textItem?.onClick &&
														'-ml-1.5 rounded-md hover:bg-app-button/50'
												)}
											>
												{Icon && (
													<Icon
														weight="fill"
														className="-mt-0.5 ml-[5px] mr-1 inline"
													/>
												)}
												{textItem?.text}
											</TextItem>

											{index < filteredItems.length - 1 && (
												<span className="truncate"> • </span>
											)}
										</Fragment>
									);
								})}
								{status == 'Running' && (
									<div className="text-[0.8rem] text-gray-400 opacity-60">
										{currentETA !== undefined
											? formatETA(currentETA)
											: 'Unable to calculate estimated completion time'}
									</div>
								)}
							</TextLine>
						</Tooltip>
					);
				})}
				<div className="mt-1">{children}</div>
			</MetaContainer>
		</li>
	);
});

export default JobContainer;
