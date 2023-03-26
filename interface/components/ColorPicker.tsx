import { HexColorInput, HexColorPicker } from 'react-colorful';
import { FieldValues, UseControllerProps, useController } from 'react-hook-form';
import { Popover, inputStyles } from '@sd/ui';

interface Props<T extends FieldValues> extends UseControllerProps<T> {
	className?: string;
}

export default <T extends FieldValues>({ className, ...props }: Props<T>) => {
	const { field } = useController({ name: props.name });

	return (
		<Popover
			trigger={
				<div className="h-4 w-4 rounded-full shadow" style={{ backgroundColor: field.value }} />
			}
			className="p-3"
			sideOffset={5}
		>
			<HexColorPicker color={field.value} onChange={field.onChange} />
			<HexColorInput
				color={field.value}
				onChange={field.onChange}
				className={inputStyles({ size: 'md', className: 'bg-app mt-5 px-3' })}
			/>
		</Popover>
	);
};
