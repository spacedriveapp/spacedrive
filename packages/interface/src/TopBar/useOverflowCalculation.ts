import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import { useTopBar, useTopBarInternal, TopBarItem, TopBarPosition } from "./Context";

const GAP = 8;
const OVERFLOW_BUTTON_WIDTH = 44;

function sortByPriority(a: TopBarItem, b: TopBarItem): number {
	const priorityOrder = { high: 0, normal: 1, low: 2 };
	return priorityOrder[a.priority] - priorityOrder[b.priority];
}

function calculateFitting(
	items: TopBarItem[],
	containerWidth: number
): { visible: TopBarItem[]; overflow: TopBarItem[] } {
	const visible: TopBarItem[] = [];
	const overflow: TopBarItem[] = [];

	let usedWidth = 0;
	let willHaveOverflow = false;

	// First pass: add all high-priority items
	for (const item of items) {
		if (item.priority === "high") {
			visible.push(item);
			usedWidth += item.width + GAP;
		}
	}

	// Second pass: add normal/low priority items until we run out of space
	for (const item of items) {
		if (item.priority === "high") continue;

		const itemWidth = item.width + GAP;
		const reservedSpace = overflow.length > 0 || willHaveOverflow ? OVERFLOW_BUTTON_WIDTH : 0;

		if (usedWidth + itemWidth + reservedSpace <= containerWidth) {
			visible.push(item);
			usedWidth += itemWidth;
		} else {
			overflow.push(item);
			willHaveOverflow = true;
		}
	}

	return { visible, overflow };
}

export function useOverflowCalculation() {
	const { items, leftContainerRef, rightContainerRef } = useTopBar();
	const { setVisibleItems, setOverflowItems, recalculationTrigger } = useTopBarInternal();
	const parentContainerRef = useRef<HTMLDivElement>(null);

	const calculateOverflow = useCallback(() => {
		if (!leftContainerRef?.current || !rightContainerRef?.current || !parentContainerRef.current) return;

		// Calculate available space based on parent container width
		const parentWidth = parentContainerRef.current.offsetWidth;
		const PADDING = 24; // px-3 = 12px on each side
		const GAPS = 24; // gap-3 between 3 sections = 12px * 2
		const availableWidth = parentWidth - PADDING - GAPS;

		// Split available space between left and right (center gets flex-1, the remainder)
		// Give each side up to 45% of available space (leaving 10% minimum for center)
		const maxSideWidth = availableWidth * 0.45;

		const leftItems = Array.from(items.values())
			.filter(item => item.position === "left")
			.sort(sortByPriority);

		const rightItems = Array.from(items.values())
			.filter(item => item.position === "right")
			.sort(sortByPriority);

		const centerItems = Array.from(items.values())
			.filter(item => item.position === "center");


		const leftResult = calculateFitting(leftItems, maxSideWidth);
		const rightResult = calculateFitting(rightItems, maxSideWidth);

		const newVisibleItems = new Set([
			...leftResult.visible.map(item => item.id),
			...rightResult.visible.map(item => item.id),
			...centerItems.map(item => item.id),
		]);

		const newOverflowItems = new Map<TopBarPosition, TopBarItem[]>([
			["left", leftResult.overflow],
			["right", rightResult.overflow],
			["center", []],
		]);

		setVisibleItems(newVisibleItems);
		setOverflowItems(newOverflowItems);
	}, [items, leftContainerRef, rightContainerRef, setVisibleItems, setOverflowItems]);

	useLayoutEffect(() => {
		calculateOverflow();
	}, [calculateOverflow, recalculationTrigger]);

	// Watch parent container size changes with ResizeObserver
	useLayoutEffect(() => {
		const parentEl = parentContainerRef.current;
		if (!parentEl) return;

		const resizeObserver = new ResizeObserver(() => {
			calculateOverflow();
		});

		resizeObserver.observe(parentEl);

		return () => {
			resizeObserver.disconnect();
		};
	}, [calculateOverflow]);

	return parentContainerRef;
}