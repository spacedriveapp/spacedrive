import clsx from 'clsx';
import { PropsWithChildren, useRef } from 'react';
import { Tooltip } from '@sd/ui';
import { useIsTextTruncated } from '~/hooks';

export const TruncatedText = ({
	children,
	className
}: PropsWithChildren<{ className?: string }>) => {
	const ref = useRef<HTMLDivElement>(null);

	const isTruncated = useIsTextTruncated(ref);

	return (
		<Tooltip tooltipClassName="max-w-fit" label={isTruncated ? children : undefined} asChild>
			<div ref={ref} className={clsx('truncate', className)}>
				{children}
			</div>
		</Tooltip>
	);
};
