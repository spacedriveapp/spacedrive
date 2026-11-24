import type { TranscriptSegment } from "@sd/ts-client";
import { describe, expect, test } from "bun:test";
import { activeSegmentIndex, formatTimestamp } from "./TranscriptPanel";

const segment = (start_ms: number, end_ms: number): TranscriptSegment => ({
	id: `seg_${start_ms}`,
	start_ms,
	end_ms,
	text: "",
});

const segments = [segment(0, 1000), segment(1500, 4000), segment(4000, 6000)];

describe("activeSegmentIndex", () => {
	test("finds the segment being spoken", () => {
		expect(activeSegmentIndex(segments, 0)).toBe(0);
		expect(activeSegmentIndex(segments, 2000)).toBe(1);
		expect(activeSegmentIndex(segments, 5999)).toBe(2);
	});

	test("hands over at the boundary", () => {
		expect(activeSegmentIndex(segments, 4000)).toBe(2);
	});

	test("answers -1 between and after segments", () => {
		expect(activeSegmentIndex(segments, 1200)).toBe(-1);
		expect(activeSegmentIndex(segments, 6000)).toBe(-1);
		expect(activeSegmentIndex([], 10)).toBe(-1);
	});
});

describe("formatTimestamp", () => {
	test("drops the hour until there is one", () => {
		expect(formatTimestamp(65_000)).toBe("1:05");
		expect(formatTimestamp(3_723_000)).toBe("1:02:03");
	});
});
