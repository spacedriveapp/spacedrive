import clsx from 'clsx';
import { PropsWithChildren } from 'react';
import DragRegion from "~/components/layout/DragRegion"

export function ScreenContainer(props: PropsWithChildren & { className?: string }) {
	return (
		<div
			className={clsx(
				'flex flex-col w-full h-screen p-5 pt-0 custom-scroll page-scroll app-background',
				props.className
			)}
		>
			<DragRegion />
			{props.children}
		</div>
	);
}
