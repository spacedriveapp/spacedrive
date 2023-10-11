import { ReactNode, useState } from 'react';

export const SEE_MORE_COUNT = 5;

interface SeeMoreProps<T> {
	items: T[];
	renderItem: (item: T, index: number) => ReactNode;
	limit?: number;
}

const SeeMore = <T,>({ items, renderItem, limit = SEE_MORE_COUNT }: SeeMoreProps<T>) => {
	const [seeMore, setSeeMore] = useState(false);

	const displayedItems = seeMore ? items : items.slice(0, limit);

	return (
		<>
			{displayedItems.map((item, index) => renderItem(item, index))}
			{items.length > limit && (
				<div
					onClick={() => setSeeMore(!seeMore)}
					className="mb-1 ml-2 mt-0.5 cursor-pointer text-center text-tiny font-semibold text-ink-faint/50 transition hover:text-accent"
				>
					See {seeMore ? 'less' : 'more'}
				</div>
			)}
		</>
	);
};

export default SeeMore;
