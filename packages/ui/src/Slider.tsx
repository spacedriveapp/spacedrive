'use client';

import * as SliderPrimitive from '@radix-ui/react-slider';
import clsx from 'clsx';

export const Slider = (props: SliderPrimitive.SliderProps) => (
	<SliderPrimitive.Root
		{...props}
		className={clsx('relative flex h-6 w-full select-none items-center', props.className)}
	>
		<SliderPrimitive.Track className="relative h-2 grow rounded-full bg-app-slider outline-none">
			<SliderPrimitive.Range className="absolute h-full rounded-full bg-accent outline-none" />
		</SliderPrimitive.Track>
		<SliderPrimitive.Thumb
			className="z-50 block size-5 rounded-full bg-accent font-bold shadow-lg shadow-black/20 outline-none ring-accent/30 transition focus:ring-4"
			data-tip="1.0"
		/>
	</SliderPrimitive.Root>
);
