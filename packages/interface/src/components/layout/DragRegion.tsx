import { cx } from '@sd/ui';

export default function DragRegion(props: { classNames?: string }) {
	return (
		<div data-tauri-drag-region className={cx('flex flex-shrink-0 w-full h-5', props.classNames)} />
	);
}
