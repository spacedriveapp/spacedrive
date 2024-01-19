import { ComponentProps, forwardRef } from 'react';

import { useSize } from './utils';

export interface ImageProps extends ComponentProps<'img'> {
	extension?: string;
	size: ReturnType<typeof useSize>;
}

export const Image = forwardRef<HTMLImageElement, ImageProps>(
	({ crossOrigin, size, ...props }, ref) => (
		<img
			// Order matter for crossOrigin attr
			// https://github.com/facebook/react/issues/14035#issuecomment-642227899
			{...(crossOrigin ? { crossOrigin } : {})}
			ref={ref}
			draggable={false}
			{...props}
		/>
	)
);
