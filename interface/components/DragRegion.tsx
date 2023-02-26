import { PropsWithChildren, forwardRef } from 'react';
import { cx } from '@sd/ui';

export default forwardRef<HTMLDivElement, PropsWithChildren & { className?: string }>(
	(props, ref) => (
		<div
			data-tauri-drag-region
			className={cx('flex h-5 w-full flex-shrink-0', props.className)}
			ref={ref}
		>
			{props.children}
		</div>
	)
);
