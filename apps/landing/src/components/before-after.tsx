'use client';

// adapted from https://github.com/unhingedmagikarp/comparison-slider/blob/main/src/app/components/Slider/index.ts
import { CaretUpDown } from '@phosphor-icons/react';
import Image, { StaticImageData } from 'next/image';
import { useState } from 'react';

interface SliderProps {
	beforeImage: string | StaticImageData;
	afterImage: string | StaticImageData;
}

export const Slider: React.FC<SliderProps> = ({ beforeImage, afterImage }) => {
	const [sliderPosition, setSliderPosition] = useState(50);
	const [isDragging, setIsDragging] = useState(false);

	const handleMove = (event: React.MouseEvent<HTMLDivElement, MouseEvent>) => {
		if (!isDragging) return;

		const rect = event.currentTarget.getBoundingClientRect();
		const x = Math.max(0, Math.min(event.clientX - rect.left, rect.width));
		const percent = Math.max(0, Math.min((x / rect.width) * 100, 100));

		setSliderPosition(percent);
	};

	const handleMouseDown = () => {
		setIsDragging(true);
	};

	const handleMouseUp = () => {
		setIsDragging(false);
	};

	return (
		<div className="relative w-full" onMouseUp={handleMouseUp}>
			<div
				className="relative m-auto aspect-[5/3] max-h-[300px] w-full max-w-[1000px] select-none overflow-hidden rounded-lg shadow-xl"
				onMouseMove={handleMove}
				onMouseDown={handleMouseDown}
			>
				<Image
					alt="Before"
					fill
					draggable={false}
					priority
					src={beforeImage}
					className="rounded-lg"
				/>

				<div
					className="absolute inset-x-0 top-0 m-auto aspect-[70/45] max-h-[300px] w-full max-w-[1000px] select-none overflow-hidden rounded-lg"
					style={{ clipPath: `inset(0 ${100 - sliderPosition}% 0 0)` }}
				>
					<Image
						fill
						priority
						draggable={false}
						alt="After"
						src={afterImage}
						className="rounded-lg"
					/>
				</div>

				{/* Slider handle */}
				<div
					className="absolute inset-y-0 w-[2px] cursor-ew-resize bg-white"
					style={{
						left: `calc(${sliderPosition}% - 1px)`
					}}
				>
					<CaretUpDown
						size={48}
						className="absolute left-[-22px] top-[calc(50%-10px)]"
						style={{ transform: 'rotate(90deg)' }}
					/>
				</div>
			</div>
		</div>
	);
};
