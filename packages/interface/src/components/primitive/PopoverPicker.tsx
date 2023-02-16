import clsx from 'clsx';
import { useCallback, useRef, useState } from 'react';
import { HexColorPicker } from 'react-colorful';
import { UseControllerProps, useController } from 'react-hook-form';
import useClickOutside from '../../hooks/useClickOutside';

interface PopoverPickerProps extends UseControllerProps {
	className?: string;
}

export const PopoverPicker = ({ className, ...props }: PopoverPickerProps) => {
	const { field } = useController(props);
	const popover = useRef<HTMLDivElement | null>(null);
	const [isOpen, toggle] = useState(false);

	const close = useCallback(() => toggle(false), []);
	useClickOutside(popover, close);

	return (
		<div className={clsx('relative mt-3 flex items-center', className)}>
			<div
				className={clsx('h-5 w-5 rounded-full shadow ', isOpen && 'dark:border-gray-500')}
				style={{ backgroundColor: field.value }}
				onClick={() => toggle(true)}
			/>
			{/* <span className="inline ml-2 text-sm text-gray-200">Pick Color</span> */}

			{isOpen && (
				<div
					style={{ top: 'calc(100% + 7px)' }}
					className="absolute left-0 rounded-md shadow"
					ref={popover}
				>
					<HexColorPicker color={field.value} onChange={field.onChange} />
				</div>
			)}
		</div>
	);
};
