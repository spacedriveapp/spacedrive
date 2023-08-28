import clsx from 'clsx';
import { ForwardRefExoticComponent, Fragment, HTMLAttributes, ReactNode, forwardRef } from 'react';
import { TextItems } from '@sd/client';
import { Tooltip, tw } from '@sd/ui';
import classes from './Job.module.scss';

interface JobContainerProps extends HTMLAttributes<HTMLLIElement> {
	name: string;
	icon?: string | ForwardRefExoticComponent<any>;
	// Array of arrays of TextItems, where each array of TextItems is a truncated line of text.
	textItems?: TextItems;
	isChild?: boolean;
	children?: ReactNode;
}

const CIRCLE_ICON_CLASS = `relative flex-shrink-0 top-1 z-20 mr-3 h-7 w-7 rounded-full bg-app-button p-[5.5px]`;
const IMG_ICON_CLASS = `relative left-[-2px] top-1 z-10 mr-2 h-8 w-8`;

const MetaContainer = tw.div`flex w-full overflow-hidden flex-col`;
const TextLine = tw.div`mt-[2px] gap-1 text-ink-faint truncate mr-8 pl-1.5`;
const TextItem = tw.span`truncate`;

// Job container consolidates the common layout of a job item, used for regular jobs (Job.tsx) and grouped jobs (JobGroup.tsx).
const JobContainer = forwardRef<HTMLLIElement, JobContainerProps>((props, ref) => {
	const { name, icon: Icon, textItems, isChild, children, className, ...restProps } = props;

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
				<Tooltip asChild tooltipClassName="bg-black max-w-[400px]" position="top" label={name}>
					<p className="truncate max-w-[83%] pl-1.5 font-semibold">{name}</p>
				</Tooltip>
				{textItems?.map((item, index) => {
					// filter out undefined text so we don't render empty TextItems
					const filteredItems = item.filter((i) => i?.text);

					const popoverText = filteredItems.map((i) => i?.text).join(' • ');

					return (
						<Tooltip
							label={popoverText}
							key={index}
							tooltipClassName="bg-black max-w-[400px]"
						>
							<TextLine>
								{filteredItems.map((textItem, index) => {
									const Icon = textItem?.icon;
									return (
										<Fragment key={index}>
											<TextItem
												onClick={textItem?.onClick}
												className={clsx(
													// index > 0 && 'px-1.5 py-0.5 italic',
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
