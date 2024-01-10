/** @jsxImportSource solid-js */

import clsx from 'clsx';
import { ParentProps } from 'solid-js';

export const TruncatedText2 = (props: ParentProps<{ className?: string }>) => {
	// TODO: Finish this

	// const ref = useRef<HTMLDivElement>(null);

	// const isTruncated = useIsTextTruncated(ref);

	// return (
	// 	<Tooltip tooltipClassName="max-w-fit" label={isTruncated ? children : undefined} asChild>
	// 		<div ref={ref} className={clsx('truncate', className)}>
	// 			{children}
	// 		</div>
	// 	</Tooltip>
	// );

	return <div class={clsx('truncate', props.className)}>{props.children}</div>;
};
