import { forwardRef, PropsWithChildren } from 'react';
import { cx } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';

export default forwardRef<HTMLDivElement, PropsWithChildren & { className?: string }>(
	(props, ref) => {
		const os = useOperatingSystem();

		return (
			<div
				data-tauri-drag-region={os === 'macOS'}
				className={cx('flex h-5 w-full flex-shrink-0', props.className)}
				ref={ref}
			>
				{props.children}
			</div>
		);
	}
);
