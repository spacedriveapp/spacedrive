import { PropsWithChildren } from 'react';
import { cx } from '@sd/ui';

export default function DragRegion(props: PropsWithChildren & { className?: string }) {
	return (
		<div data-tauri-drag-region className={cx('flex h-5 w-full flex-shrink-0', props.className)}>
			{props.children}
		</div>
	);
}
