import clsx from 'clsx';
import { DefaultProps } from './types';

export interface ShortcutProps extends DefaultProps {
	chars: string;
}

export const Shortcut: React.FC<ShortcutProps> = (props) => {
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
