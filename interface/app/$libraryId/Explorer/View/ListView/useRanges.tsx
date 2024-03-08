import { Row } from '@tanstack/react-table';
import { useCallback } from 'react';
import { type ExplorerItem } from '@sd/client';

export function getRangeDirection(start: number, end: number) {
	return start < end ? ('down' as const) : start > end ? ('up' as const) : null;
}

export type Range = [string, string];

interface UseRangesProps {
	ranges: Range[];
	rows: Record<string, Row<ExplorerItem>>;
}

export const useRanges = ({ ranges, rows }: UseRangesProps) => {
	const getRangeRows = useCallback(
		(range: Range) => {
			const rangeRows = range
				.map((id) => rows[id])
				.filter((row): row is Row<ExplorerItem> => Boolean(row));

			const [start, end] = rangeRows;

			const [sortedStart, sortedEnd] = [...rangeRows].sort((a, b) => a.index - b.index);

			if (!start || !end || !sortedStart || !sortedEnd) return;

			return { start, end, sorted: { start: sortedStart, end: sortedEnd } };
		},
		[rows]
	);

	const getRangeByIndex = useCallback(
		(index: number) => {
			const range = ranges[index];

			if (!range) return;

			const rangeRows = getRangeRows(range);

			if (!rangeRows) return;

			const direction = getRangeDirection(rangeRows.start.index, rangeRows.end.index);

			return { ...rangeRows, direction, index };
		},
		[getRangeRows, ranges]
	);

	const getRangesByRow = useCallback(
		({ index }: Row<ExplorerItem>) => {
			const _ranges = ranges.reduce<NonNullable<ReturnType<typeof getRangeByIndex>>[]>(
				(ranges, range, i) => {
					const rangeRows = getRangeRows(range);

					if (!rangeRows) return ranges;

					if (
						index >= rangeRows.sorted.start.index &&
						index <= rangeRows.sorted.end.index
					) {
						const range = getRangeByIndex(i);
						return range ? [...ranges, range] : ranges;
					}

					return ranges;
				},
				[]
			);

			return _ranges;
		},
		[getRangeByIndex, getRangeRows, ranges]
	);

	const sortRanges = useCallback(
		(ranges: Range[]) => {
			return ranges
				.map((range, i) => {
					const rows = getRangeRows(range);

					if (!rows) return;

					return {
						index: i,
						...rows
					};
				})
				.filter(
					(
						range
					): range is NonNullable<ReturnType<typeof getRangeRows>> & { index: number } =>
						Boolean(range)
				)
				.sort((a, b) => a.sorted.start.index - b.sorted.start.index);
		},
		[getRangeRows]
	);

	const getClosestRange = useCallback(
		(
			rangeIndex: number,
			options: {
				direction?: 'up' | 'down';
				maxRowDifference?: number;
				ranges?: Range[];
			} = {}
		) => {
			const range = getRangeByIndex(rangeIndex);

			let _ranges = sortRanges(options.ranges || ranges);

			if (range) {
				_ranges = _ranges.filter(
					(_range) =>
						range.index === _range.index ||
						range.sorted.start.index < _range.sorted.start.index ||
						range.sorted.end.index > _range.sorted.end.index
				);
			}

			const targetRangeIndex = _ranges.findIndex(({ index }) => rangeIndex === index);

			const targetRange = _ranges[targetRangeIndex];

			if (!targetRange) return;

			const closestRange =
				options.direction === 'down'
					? _ranges[targetRangeIndex + 1]
					: options.direction === 'up'
						? _ranges[targetRangeIndex - 1]
						: _ranges[targetRangeIndex + 1] || _ranges[targetRangeIndex - 1];

			if (!closestRange) return;

			const direction = options.direction || (_ranges[targetRangeIndex + 1] ? 'down' : 'up');

			const rowDifference =
				direction === 'down'
					? closestRange.sorted.start.index - 1 - targetRange.sorted.end.index
					: targetRange.sorted.start.index - (closestRange.sorted.end.index + 1);

			if (options.maxRowDifference !== undefined && rowDifference > options.maxRowDifference)
				return;

			return {
				...closestRange,
				direction,
				rowDifference
			};
		},
		[getRangeByIndex, ranges, sortRanges]
	);

	return { getRangeByIndex, getRangesByRow, getClosestRange };
};
