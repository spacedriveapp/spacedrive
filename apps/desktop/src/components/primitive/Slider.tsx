import React from 'react';
import * as SliderPrimitive from '@radix-ui/react-slider';

const Slider = (props: SliderPrimitive.SliderProps) => (
  <SliderPrimitive.Root {...props} className="relative flex items-center w-full h-6 select-none">
    <SliderPrimitive.Track className="relative flex-grow h-2 bg-gray-500 rounded-full outline-none">
      <SliderPrimitive.Range className="absolute h-full rounded-full outline-none bg-primary-500" />
    </SliderPrimitive.Track>
    <SliderPrimitive.Thumb
      className="z-50 block w-5 h-5 font-bold transition rounded-full shadow-lg outline-none shadow-black bg-primary-500 ring-primary-500 ring-opacity-30 focus:ring-4"
      data-tip="1.0"
    />
  </SliderPrimitive.Root>
);

export default Slider;
