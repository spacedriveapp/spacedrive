import clsx from 'clsx';
import { ComponentProps } from 'react';

export interface ShortcutProps extends ComponentProps<'div'> {
	chars: string;
}

export const Shortcut = (props: ShortcutProps) => {
	const { className, chars, ...rest } = props;

	return (
		<kbd
			className={clsx(
				`border border-b-2 px-1`,
				`font-ink-dull rounded-md text-xs font-bold`,
				`border-app-line dark:border-transparent`,
				className
			)}
			{...rest}
		>
			{chars}
		</kbd>
	);
};
