import clsx from 'clsx';
import { PropsWithChildren, ReactNode } from 'react';
import DragRegion from '~/components/layout/DragRegion';

export function ScreenContainer(
	props: PropsWithChildren & { className?: string; dragRegionChildren?: ReactNode }
) {
	return (
		<div
			className={clsx(
				'flex flex-col w-full h-screen custom-scroll page-scroll app-background',
				props.className
			)}
		>
			<DragRegion>{props.dragRegionChildren}</DragRegion>

			<div className="flex flex-col w-full h-screen p-5 pt-0">{props.children}</div>
		</div>
	);
}
