import clsx from 'clsx';

import HorizontalScroll from './HorizontalScroll';

const COUNT_STYLE = `min-w-[20px] flex h-[20px] px-1 items-center justify-center rounded-full border border-app-button/40 text-[9px]`;

const OverviewSection = ({
	children,
	title,
	className,
	count
}: React.HTMLAttributes<HTMLDivElement> & { title?: string; count?: number }) => {
	return (
		<div className={clsx('group w-full', className)}>
			{title && (
				<div className="mb-3 flex w-full items-center gap-3 px-7">
					<div className="truncate font-plex font-bold">{title}</div>
					{typeof count === 'number' && <div className={COUNT_STYLE}>{count}</div>}
					<div className="grow" />
					<div className="flex flex-row gap-1 text-sidebar-inkFaint opacity-0 transition-all duration-300 hover:!opacity-100 group-hover:opacity-30">
						{/* <Button className={BUTTON_STYLE} size="icon" variant="subtle">
							<CaretUp weight="fill" className="w-3 h-3 text-ink-faint " />
							</Button>
							<Button className={BUTTON_STYLE} size="icon" variant="subtle">
							<CaretDown weight="fill" className="w-3 h-3 text-ink-faint " />
						</Button> */}
						{/* <Button className={BUTTON_STYLE} size="icon" variant="subtle">
							<Ellipsis className="w-3 h-3 text-ink-faint " />
						</Button> */}
					</div>
				</div>
			)}
			{/* {title && <div className="mx-7 mb-3 h-[1px] w-full bg-app-line/50" />} */}

			<HorizontalScroll>{children}</HorizontalScroll>
			<div className="my-2 h-px w-full" />
		</div>
	);
};

export default OverviewSection;
