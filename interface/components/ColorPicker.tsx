import clsx from 'clsx';
import { HexColorInput, HexColorPicker } from 'react-colorful';
import { FieldValues, useController, UseControllerProps } from 'react-hook-form';
import { inputStyles, Popover, usePopover } from '@sd/ui';

interface Props<T extends FieldValues> extends UseControllerProps<T> {
	className?: string;
}

export const ColorPicker = <T extends FieldValues>({ className, ...props }: Props<T>) => {
	const { field } = useController({ name: props.name });

	return (
		<Popover
			popover={usePopover()}
			trigger={
				<div
					className={clsx('size-4 rounded-full shadow', className)}
					style={{ backgroundColor: field.value }}
				/>
			}
			className="relative z-[110] p-3"
			sideOffset={5}
		>
			<HexColorPicker color={field.value} onChange={field.onChange} />
			<HexColorInput
				color={field.value}
				onChange={field.onChange}
				className={inputStyles({ size: 'md', className: '!mt-5 bg-app px-3' })}
			/>
		</Popover>
	);
};
