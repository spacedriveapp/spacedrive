import * as SliderPrimitive from '@radix-ui/react-slider';
import clsx from 'clsx';

const Slider = (props: SliderPrimitive.SliderProps) => (
	<SliderPrimitive.Root
		{...props}
		className={clsx('relative flex items-center w-full h-6 select-none', props.className)}
	>
		<SliderPrimitive.Track className="relative flex-grow h-2 rounded-full outline-none bg-app-box">
			<SliderPrimitive.Range className="absolute h-full rounded-full outline-none bg-accent" />
		</SliderPrimitive.Track>
		<SliderPrimitive.Thumb
			className="z-50 block w-5 h-5 font-bold transition rounded-full shadow-lg outline-none shadow-black/20 bg-accent ring-accent ring-opacity-30 focus:ring-4"
			data-tip="1.0"
		/>
	</SliderPrimitive.Root>
);

export default Slider;
