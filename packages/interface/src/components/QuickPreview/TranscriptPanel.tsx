import type { TranscriptSegment } from "@sd/ts-client";
import { useVirtualizer } from "@tanstack/react-virtual";
import clsx from "clsx";
import { useCallback, useEffect, useRef, useState } from "react";

export const TRANSCRIPT_PANEL_WIDTH = 320;

/** How long a manual scroll keeps the panel from following playback. */
const MANUAL_SCROLL_HOLD_MS = 3000;

/**
 * The index of the segment being spoken at `timeMs`, or -1 between segments.
 * Segments are ordered and never overlap, so a binary search finds it.
 */
export function activeSegmentIndex(
	segments: TranscriptSegment[],
	timeMs: number,
): number {
	let low = 0;
	let high = segments.length - 1;
	while (low <= high) {
		const mid = (low + high) >> 1;
		const segment = segments[mid];
		if (timeMs < segment.start_ms) high = mid - 1;
		else if (timeMs >= segment.end_ms) low = mid + 1;
		else return mid;
	}
	return -1;
}

/** The playback position of a media element in milliseconds, as it changes. */
export function useMediaTimeMs(media: HTMLMediaElement | null): number {
	const [timeMs, setTimeMs] = useState(0);
	useEffect(() => {
		if (!media) return;
		const update = () => setTimeMs(media.currentTime * 1000);
		update();
		media.addEventListener("timeupdate", update);
		media.addEventListener("seeked", update);
		return () => {
			media.removeEventListener("timeupdate", update);
			media.removeEventListener("seeked", update);
		};
	}, [media]);
	return timeMs;
}

export function formatTimestamp(ms: number): string {
	const total = Math.floor(ms / 1000);
	const hours = Math.floor(total / 3600);
	const mins = Math.floor((total % 3600) / 60);
	const secs = (total % 60).toString().padStart(2, "0");
	return hours > 0
		? `${hours}:${mins.toString().padStart(2, "0")}:${secs}`
		: `${mins}:${secs}`;
}

interface TranscriptPanelProps {
	segments: TranscriptSegment[];
	media: HTMLMediaElement | null;
}

/**
 * A recording's transcript beside its player. Clicking a line seeks to it,
 * and the line being spoken stays in view unless the reader has just
 * scrolled away from it. Long recordings run to thousands of lines, so only
 * the visible ones are mounted.
 */
export function TranscriptPanel({ segments, media }: TranscriptPanelProps) {
	const timeMs = useMediaTimeMs(media);
	const active = activeSegmentIndex(segments, timeMs);
	const listRef = useRef<HTMLDivElement>(null);
	const lastManualScroll = useRef(0);
	const holdFollow = () => {
		lastManualScroll.current = Date.now();
	};
	// A new key function invalidates the virtualizer's measurements, which
	// re-renders during render, so it changes only with the segments.
	const getItemKey = useCallback(
		(index: number) => segments[index].id,
		[segments],
	);
	const virtualizer = useVirtualizer({
		count: segments.length,
		getScrollElement: () => listRef.current,
		estimateSize: () => 44,
		getItemKey,
		paddingEnd: 96,
	});

	useEffect(() => {
		if (active < 0) return;
		if (Date.now() - lastManualScroll.current < MANUAL_SCROLL_HOLD_MS) return;
		virtualizer.scrollToIndex(active, { align: "center" });
	}, [active, virtualizer]);

	return (
		<div
			className="flex h-full shrink-0 flex-col border-l border-white/10 bg-black/60 backdrop-blur-xl"
			style={{ width: TRANSCRIPT_PANEL_WIDTH }}
		>
			<div className="px-4 pb-2 pt-3 text-xs font-semibold uppercase tracking-wide text-white/50">
				Transcript
			</div>
			<div
				ref={listRef}
				className="flex-1 overflow-y-auto px-2"
				// Scroll events can't tell the reader from follow-along, so watch the
				// inputs that scroll: wheel, touch, keys, and the scrollbar, whose
				// presses land on the list itself rather than on a line.
				onWheel={holdFollow}
				onTouchMove={holdFollow}
				onKeyDown={holdFollow}
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) holdFollow();
				}}
			>
				<div
					className="relative w-full"
					style={{ height: virtualizer.getTotalSize() }}
				>
					{virtualizer.getVirtualItems().map((item) => {
						const segment = segments[item.index];
						const isActive = item.index === active;
						return (
							<button
								key={item.key}
								ref={virtualizer.measureElement}
								data-index={item.index}
								type="button"
								onClick={() => {
									if (!media) return;
									media.currentTime = segment.start_ms / 1000;
									lastManualScroll.current = 0;
								}}
								className={clsx(
									"absolute left-0 top-0 flex w-full gap-3 rounded-md px-2 py-1.5 text-left text-sm transition-colors",
									isActive
										? "bg-accent/20 text-white"
										: "text-white/60 hover:bg-white/5 hover:text-white/90",
								)}
								style={{ transform: `translateY(${item.start}px)` }}
							>
								<span
									className={clsx(
										"shrink-0 pt-px text-xs tabular-nums",
										isActive ? "text-accent" : "text-white/40",
									)}
								>
									{formatTimestamp(segment.start_ms)}
								</span>
								<span className="leading-snug">{segment.text}</span>
							</button>
						);
					})}
				</div>
			</div>
		</div>
	);
}
