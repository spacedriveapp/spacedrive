import * as SliderPrimitive from '@radix-ui/react-slider';
import clsx from 'clsx';

const Slider = (props: SliderPrimitive.SliderProps) => (
	<SliderPrimitive.Root
		{...props}
		className={clsx('relative flex h-6 w-full select-none items-center', props.className)}
	>
		<SliderPrimitive.Track className="bg-app-box relative h-2 flex-grow rounded-full outline-none">
			<SliderPrimitive.Range className="bg-accent absolute h-full rounded-full outline-none" />
		</SliderPrimitive.Track>
		<SliderPrimitive.Thumb
			className="bg-accent ring-accent z-50 block h-5 w-5 rounded-full font-bold shadow-lg shadow-black/20 outline-none ring-opacity-30 transition focus:ring-4"
			data-tip="1.0"
		/>
	</SliderPrimitive.Root>
);

export default Slider;
