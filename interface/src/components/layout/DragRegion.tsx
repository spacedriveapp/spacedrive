import { PropsWithChildren, forwardRef } from 'react';
import { cx } from '@sd/ui';

export default forwardRef((props: PropsWithChildren & { className?: string }) => {
	return (
		<div data-tauri-drag-region className={cx('flex h-5 w-full flex-shrink-0', props.className)}>
			{props.children}
		</div>
	);
});
