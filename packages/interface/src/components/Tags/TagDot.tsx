import clsx from 'clsx';

interface TagDotProps {
	color: string;
	tooltip?: string;
	onClick?: (e: React.MouseEvent) => void;
	className?: string;
}

/**
 * Small colored circle indicator for tag visualization (6px)
 * Used in FileCard and compact layouts
 */
export function TagDot({ color, tooltip, onClick, className }: TagDotProps) {
	const Component = onClick ? 'button' : 'span';

	return (
		<Component
			className={clsx(
				'size-1.5 rounded-full',
				onClick && 'hover:scale-125 transition-transform cursor-pointer',
				className
			)}
			style={{ backgroundColor: color }}
			title={tooltip}
			onClick={onClick}
		/>
	);
}
