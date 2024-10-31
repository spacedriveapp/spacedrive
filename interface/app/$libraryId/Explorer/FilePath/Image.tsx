import { ComponentProps, forwardRef } from 'react';

export const Image = forwardRef<HTMLImageElement, ComponentProps<'img'>>(
	({ crossOrigin, ...props }, ref) => (
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
