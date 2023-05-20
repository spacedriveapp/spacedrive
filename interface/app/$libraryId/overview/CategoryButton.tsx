import clsx from 'clsx';

interface CategoryButtonProps {
	category: string;
	items: number;
	icon: string;
	selected?: boolean;
	onClick?: () => void;
}

export default ({ category, icon, items, selected, onClick }: CategoryButtonProps) => {
	return (
		<div
			onClick={onClick}
			className={clsx(
				'flex shrink-0 items-center rounded-md px-1.5 py-1 text-sm',
				selected && 'bg-app-selectedItem'
			)}
		>
			<img src={icon} className="mr-3 h-12 w-12" />
			<div className="pr-5">
				<h2 className="text-sm font-medium">{category}</h2>
				{items !== undefined && (
					<p className="text-xs text-ink-faint">
						{numberWithCommas(items)} Item{(items > 1 || items === 0) && 's'}
					</p>
				)}
			</div>
		</div>
	);
};

function numberWithCommas(x: number) {
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}
