import { CaretDown, CaretUp } from '@phosphor-icons/react';
import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import clsx from 'clsx';
import { Button } from '@sd/ui';

import HorizontalScroll from './HorizontalScroll';

const COUNT_STYLE = `min-w-[20px] flex h-[20px] px-1 items-center justify-center rounded-full border border-app-button/40 text-[9px]`;

const BUTTON_STYLE = `!p-[5px] opacity-0 transition-opacity group-hover:opacity-100`;

const OverviewSection = ({
	children,
	title,
	className,
	count
}: React.HTMLAttributes<HTMLDivElement> & { title?: string; count?: number }) => {
	return (
		<div className={clsx('group w-full', className)}>
			{title && (
				<div className="mb-3 flex w-full items-center gap-3 px-7 ">
					<div className="truncate font-bold">{title}</div>
					{count && <div className={COUNT_STYLE}>{count}</div>}
					<div className="grow" />
					<div className="flex flex-row gap-1 text-sidebar-inkFaint opacity-0 transition-all duration-300 hover:!opacity-100 group-hover:opacity-30">
						{/* <Button className={BUTTON_STYLE} size="icon" variant="subtle">
							<CaretUp weight="fill" className="h-3 w-3 text-ink-faint " />
							</Button>
							<Button className={BUTTON_STYLE} size="icon" variant="subtle">
							<CaretDown weight="fill" className="h-3 w-3 text-ink-faint " />
						</Button> */}
						<Button className={BUTTON_STYLE} size="icon" variant="subtle">
							<Ellipsis className="h-3 w-3 text-ink-faint " />
						</Button>
					</div>
				</div>
			)}
			{/* {title && <div className="mx-7 mb-3 h-[1px] w-full bg-app-line/50" />} */}

			<HorizontalScroll>{children}</HorizontalScroll>
			<div className="my-2 h-[1px] w-full " />
		</div>
	);
};

export default OverviewSection;
