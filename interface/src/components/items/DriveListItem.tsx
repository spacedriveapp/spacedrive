import clsx from 'clsx';
import { DefaultProps } from '../primitive/types';

export interface DriveListItemProps extends DefaultProps {
	name: string;
}

export const DriveListItem: React.FC<DriveListItemProps> = (props) => {
	return (
		<div
			className={clsx(
				'inline-block cursor-default rounded px-1.5 py-1 text-xs font-medium',
				props.className
			)}
		></div>
	);
};
