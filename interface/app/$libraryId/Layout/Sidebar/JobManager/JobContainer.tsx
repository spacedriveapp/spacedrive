/* eslint-disable tailwindcss/classnames-order */
import { tw } from '@sd/ui';
import { forwardRef, HTMLAttributes, Fragment, ForwardRefExoticComponent, ReactNode } from 'react';
import classes from './Job.module.scss';
import clsx from 'clsx';
export interface TextItem {
	text?: string;
	tooltip?: string;
	icon?: ForwardRefExoticComponent<any>;
	onClick?: () => void;
}

// first array for lines, second array for items separated by " • ".
export type TextItems = (TextItem | undefined)[][];
interface JobContainerProps extends HTMLAttributes<HTMLLIElement> {
	name: string;
	iconImg?: string;
	circleIcon?: ForwardRefExoticComponent<any>;
	textItems?: TextItems;
	isChild?: boolean;
	children?: ReactNode;
}

const CIRCLE_ICON_CLASS = `relative flex-shrink-0 top-2 z-20 mr-3 h-6 w-6 rounded-full bg-app-button p-[5.5px]`;
const IMG_ICON_CLASS = `relative left-[-2px] top-1 z-10 mr-3 h-8 w-8`;

const MetaContainer = tw.div`flex w-full flex-col`;
const TextLine = tw.div`mt-[2px] gap-1 text-ink-faint truncate mr-8`;
const TextItem = tw.span`truncate`;

// Job container consolidates the common layout of a job item, used for regular jobs (Job.tsx) and grouped jobs (JobGroup.tsx).
const JobContainer = forwardRef<HTMLLIElement, JobContainerProps>((props, ref) => {
	const {
		name,
		iconImg,
		circleIcon: CircleIcon,
		textItems,
		isChild,
		children,
		className,
		...restProps
	} = props;

	return (
		<li
			ref={ref}
			className={clsx(
				"relative flex border-b border-app-line/50 px-4 py-3",
				isChild && classes.jobGroupChild,
				isChild && "border-none bg-app-darkBox p-2 pl-10",
				className
			)}
			{...restProps}
		>
			{CircleIcon && <CircleIcon weight="fill" className={CIRCLE_ICON_CLASS} />}
			{iconImg && (<img src={iconImg} className={IMG_ICON_CLASS} />)}
			<MetaContainer>
				<span className="truncate font-semibold">{name}</span>
				{textItems?.map((textItems, lineIndex) => {
					const filteredItems = textItems.filter(i => i?.text);
					return (
						<TextLine key={lineIndex}>
							{filteredItems.map((textItem, index) => {
								const Icon = textItem?.icon;
								return (
									<Fragment key={index}>
										<TextItem
											onClick={textItem?.onClick}
											className={clsx(
												lineIndex > 0 && "italic py-0.5 px-1.5",
												textItem?.onClick && "rounded-md hover:bg-app-button/50"
											)}>
											{Icon &&
												<Icon weight="fill" className="-mt-0.5 ml-[-2px] mr-1 inline" />
											}
											{textItem?.text}
										</TextItem>
										{index < filteredItems.length - 1 && (
											<span className="truncate"> • </span>
										)}
									</Fragment>
								)
							})}
						</TextLine>
					);
				})}
				<div className="mt-1">{children}</div>
			</MetaContainer>
		</li>
	);
});

export default JobContainer;
