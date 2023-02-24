import { PropsWithChildren } from 'react';
import { cx } from '@sd/ui';

export default function DragRegion(props: PropsWithChildren & { className?: string }) {
	return (
		<div data-tauri-drag-region className={cx('flex flex-shrink-0 w-full h-5', props.className)}>
			{props.children}
		</div>
	);
}
