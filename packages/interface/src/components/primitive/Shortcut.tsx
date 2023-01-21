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
				`px-1 border border-b-2`,
				`rounded-md text-xs font-bold`,
				`border-app-line dark:border-transparent`,
				className
			)}
			{...rest}
		>
			{chars}
		</kbd>
	);
};
