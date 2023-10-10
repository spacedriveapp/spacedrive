import clsx from 'clsx';
import { formatNumber } from '@sd/client';

interface CategoryButtonProps {
	category: string;
	items: number;
	icon: string;
	selected?: boolean;
	onClick?: () => void;
	disabled?: boolean;
}

export default ({ category, icon, items, selected, onClick, disabled }: CategoryButtonProps) => {
	return (
		<div
			onClick={onClick}
			className={clsx(
				'flex shrink-0 items-center rounded-lg px-1.5 py-1 text-sm outline-none focus:bg-app-selectedItem/50',
				selected && 'bg-app-selectedItem',
				disabled && 'cursor-not-allowed opacity-30'
			)}
		>
			<img src={icon} className="mr-3 h-12 w-12" />
			<div className="pr-5">
				<h2 className="text-sm font-medium">{category}</h2>
				{items !== undefined && (
					<p className="text-xs text-ink-faint">
						{formatNumber(items)} Item{(items > 1 || items === 0) && 's'}
					</p>
				)}
			</div>
		</div>
	);
};
