import clsx from 'clsx';
import { PropsWithChildren, ReactNode, createContext } from 'react';
import DragRegion from '~/components/layout/DragRegion';

export function ScreenContainer(
	props: PropsWithChildren & { className?: string; dragRegionChildren?: ReactNode }
) {
	return (
		<div
			className={clsx(
				'custom-scroll page-scroll app-background flex h-screen w-full flex-col',
				props.className
			)}
		>
			<DragRegion>{props.dragRegionChildren}</DragRegion>
			<div className="flex h-screen w-full flex-col p-5 pt-0">{props.children}</div>
		</div>
	);
}
