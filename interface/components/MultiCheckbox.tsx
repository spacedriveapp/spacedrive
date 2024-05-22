import { Check } from '@phosphor-icons/react';
import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { useRef } from 'react';
import { CheckBox } from '@sd/ui';
import { useScrolled } from '~/hooks/useScrolled';

import { Menu } from './Menu';

interface Item {
	name: string;
	color?: string;
	icon?: React.ReactNode;
	id: number;
	selected: boolean;
}

interface SelectorProps {
	items?: Item[];
	headerArea?: React.ReactNode;
}

export default ({ items, headerArea }: SelectorProps) => {
	const parentRef = useRef<HTMLDivElement>(null);

	const rowVirtualizer = useVirtualizer({
		count: items?.length || 0,
		getScrollElement: () => parentRef.current,
		estimateSize: () => 30,
		paddingStart: 2
	});

	const { isScrolled } = useScrolled(parentRef, 10);

	return (
		<>
			{headerArea && (
				<>
					{headerArea}
					<Menu.Separator
						className={clsx('mx-0 mb-0 transition', isScrolled && 'shadow')}
					/>
				</>
			)}
			{items && items.length > 0 ? (
				<div
					ref={parentRef}
					style={{
						maxHeight: `400px`,
						height: `100%`,
						width: `100%`,
						overflow: 'auto'
					}}
				>
					<div
						style={{
							height: `${rowVirtualizer.getTotalSize()}px`,
							width: '100%',
							position: 'relative'
						}}
					>
						{rowVirtualizer.getVirtualItems().map((virtualRow) => {
							const item = items[virtualRow.index];

							if (!item) return null;
							return (
								<Menu.Item
									key={virtualRow.index}
									style={{
										position: 'absolute',
										top: 0,
										left: 0,
										width: '100%',
										height: `${virtualRow.size}px`,
										transform: `translateY(${virtualRow.start}px)`
									}}
									onClick={async (e) => {
										e.preventDefault();
									}}
								>
									{item.color && (
										<div
											className="mr-0.5 size-[15px] shrink-0 rounded-full border"
											style={{
												backgroundColor: item.selected
													? item.color
													: 'transparent',
												borderColor: item.color || '#efefef'
											}}
										/>
									)}
									{item.icon}
									{!item.color && !item.icon && (
										<CheckBox checked={item.selected} />
									)}
									<span className="truncate">{item.name}</span>
								</Menu.Item>
							);
						})}
					</div>
				</div>
			) : (
				<div className="py-1 text-center text-xs text-ink-faint">
					{items ? 'No item' : 'Failed to load items'}
				</div>
			)}
		</>
	);
};
