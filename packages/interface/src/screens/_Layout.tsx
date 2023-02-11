import clsx from 'clsx';
import { PropsWithChildren } from 'react';

export function ScreenContainer(props: PropsWithChildren & { className?: string }) {
	return (
		<div
			className={clsx(
				'flex flex-col w-full h-screen p-5 pt-0 custom-scroll page-scroll app-background',
				props.className
			)}
		>
			<div data-tauri-drag-region className="flex flex-shrink-0 w-full h-5" />
			{props.children}
		</div>
	);
}
