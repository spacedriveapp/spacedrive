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
	const SAFETY_MARGIN = 60; // Extra buffer to prevent items from going off-screen

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

		if (usedWidth + itemWidth + reservedSpace + SAFETY_MARGIN <= containerWidth) {
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

	const lastVisibleRef = useRef<Set<string>>(new Set());
	const lastOverflowRef = useRef<Map<TopBarPosition, TopBarItem[]>>(new Map());

	const calculateOverflow = useCallback(() => {
		if (!leftContainerRef?.current || !rightContainerRef?.current || !parentContainerRef.current) return;

		const parentWidth = parentContainerRef.current.offsetWidth;
		const PADDING = 24; // px-3 = 12px on each side
		const SECTION_GAPS = 24; // gap-3 between 3 sections = 12px * 2

		// Calculate how much space each side can use
		// We split available width: left takes what it needs, right takes what it needs,
		// center (flex-1) takes the rest
		const totalAvailable = parentWidth - PADDING - SECTION_GAPS;

		const leftItems = Array.from(items.values())
			.filter(item => item.position === "left")
			.sort(sortByPriority);

		const rightItems = Array.from(items.values())
			.filter(item => item.position === "right")
			.sort(sortByPriority);

		const centerItems = Array.from(items.values())
			.filter(item => item.position === "center");

		// Each side gets up to 45% of total space, but we need to account for the overflow button
		// when items start overflowing
		const maxSideWidth = totalAvailable * 0.45;

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

		// Only update if visible items actually changed
		const visibleChanged =
			newVisibleItems.size !== lastVisibleRef.current.size ||
			!Array.from(newVisibleItems).every(id => lastVisibleRef.current.has(id));

		// Only update if overflow items actually changed
		const overflowChanged =
			leftResult.overflow.length !== (lastOverflowRef.current.get("left")?.length ?? 0) ||
			rightResult.overflow.length !== (lastOverflowRef.current.get("right")?.length ?? 0);

		if (visibleChanged) {
			lastVisibleRef.current = newVisibleItems;
			setVisibleItems(newVisibleItems);
		}

		if (overflowChanged) {
			lastOverflowRef.current = newOverflowItems;
			setOverflowItems(newOverflowItems);
		}
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