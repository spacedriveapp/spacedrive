import { Columns, GridFour, Icon, MonitorPlay, Rows } from '@phosphor-icons/react';
import { isValidElement, ReactNode } from 'react';

import { useExplorerContext } from '../Context';

export const EmptyNotice = (props: {
	icon?: Icon | ReactNode;
	message?: ReactNode;
	loading?: boolean;
}) => {
	const { layoutMode } = useExplorerContext().useSettingsSnapshot();

	const emptyNoticeIcon = (icon?: Icon) => {
		const Icon =
			icon ??
			{
				grid: GridFour,
				media: MonitorPlay,
				columns: Columns,
				list: Rows
			}[layoutMode];

		return <Icon size={100} opacity={0.3} />;
	};

	if (props.loading) return null;

	return (
		<div className="flex h-full flex-col items-center justify-center text-ink-faint">
			{props.icon
				? isValidElement(props.icon)
					? props.icon
					: emptyNoticeIcon(props.icon as Icon)
				: emptyNoticeIcon()}

			<p className="mt-5">
				{props.message !== undefined ? props.message : 'This list is empty'}
			</p>
		</div>
	);
};
