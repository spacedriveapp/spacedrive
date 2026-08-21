import { useMemo } from "react";
import type { SizeNode } from "@sd/ts-client";

/**
 * Two-ring sunburst rendered from a SizeNode tree — pure SVG arcs, no
 * charting library. The inner ring is the node's children, the outer ring
 * each child's children scoped to its parent's arc. Aggregated "other"
 * bytes render as muted slices so the wheel always sums to the truth.
 */

const PALETTE = [
	"#5B8DEF",
	"#9B7BEF",
	"#EF7BA8",
	"#EFA35B",
	"#4FC1A6",
	"#E5C558",
	"#6ECFF6",
	"#C98BDB",
	"#8BD17C",
	"#E58888",
];

const OTHER_COLOR = "hsla(220, 10%, 55%, 0.35)";
const MIN_FRACTION = 0.004;

type Slice = {
	path: string;
	label: string;
	color: string;
	start: number; // fraction 0..1
	end: number;
	ring: 0 | 1;
	isDir: boolean;
	size: number;
};

function arcPath(
	cx: number,
	cy: number,
	r0: number,
	r1: number,
	startFrac: number,
	endFrac: number,
): string {
	const a0 = startFrac * Math.PI * 2 - Math.PI / 2;
	const a1 = endFrac * Math.PI * 2 - Math.PI / 2;
	const large = endFrac - startFrac > 0.5 ? 1 : 0;
	const x0o = cx + r1 * Math.cos(a0);
	const y0o = cy + r1 * Math.sin(a0);
	const x1o = cx + r1 * Math.cos(a1);
	const y1o = cy + r1 * Math.sin(a1);
	const x0i = cx + r0 * Math.cos(a1);
	const y0i = cy + r0 * Math.sin(a1);
	const x1i = cx + r0 * Math.cos(a0);
	const y1i = cy + r0 * Math.sin(a0);
	return [
		`M ${x0o} ${y0o}`,
		`A ${r1} ${r1} 0 ${large} 1 ${x1o} ${y1o}`,
		`L ${x0i} ${y0i}`,
		`A ${r0} ${r0} 0 ${large} 0 ${x1i} ${y1i}`,
		"Z",
	].join(" ");
}

export function buildSlices(root: SizeNode): Slice[] {
	const total = Math.max(
		root.size,
		root.children.reduce((a, c) => a + c.size, 0) + root.other_size,
	);
	if (total === 0) return [];

	const slices: Slice[] = [];
	let cursor = 0;

	root.children.forEach((child, i) => {
		const frac = child.size / total;
		if (frac < MIN_FRACTION) {
			cursor += frac;
			return;
		}
		const color = PALETTE[i % PALETTE.length];
		const start = cursor;
		const end = cursor + frac;
		slices.push({
			path: child.path,
			label: child.name,
			color,
			start,
			end,
			ring: 0,
			isDir: child.is_dir,
			size: child.size,
		});

		// Outer ring: this child's children within its arc.
		if (child.is_dir && child.size > 0) {
			let inner = start;
			for (const gc of child.children) {
				const gfrac = (gc.size / child.size) * frac;
				if (gfrac >= MIN_FRACTION) {
					slices.push({
						path: gc.path,
						label: gc.name,
						color,
						start: inner,
						end: inner + gfrac,
						ring: 1,
						isDir: gc.is_dir,
						size: gc.size,
					});
				}
				inner += gfrac;
			}
			if (child.other_size > 0) {
				const ofrac = (child.other_size / child.size) * frac;
				if (ofrac >= MIN_FRACTION) {
					slices.push({
						path: child.path,
						label: "other",
						color: OTHER_COLOR,
						start: inner,
						end: inner + ofrac,
						ring: 1,
						isDir: false,
						size: child.other_size,
					});
				}
			}
		}
		cursor = end;
	});

	if (root.other_size > 0) {
		const ofrac = root.other_size / total;
		slices.push({
			path: root.path,
			label: "other",
			color: OTHER_COLOR,
			start: cursor,
			end: cursor + ofrac,
			ring: 0,
			isDir: false,
			size: root.other_size,
		});
	}

	return slices;
}

interface SunburstProps {
	root: SizeNode;
	size: number;
	/** Descend into a directory slice. */
	onSelect?: (path: string, isDir: boolean) => void;
	onHover?: (slice: { label: string; size: number } | null) => void;
}

export function Sunburst({ root, size, onSelect, onHover }: SunburstProps) {
	const slices = useMemo(() => buildSlices(root), [root]);
	const c = size / 2;
	const r0 = size * 0.18;
	const r1 = size * 0.32;
	const r2 = size * 0.47;

	return (
		<svg
			width={size}
			height={size}
			viewBox={`0 0 ${size} ${size}`}
			role="img"
			aria-label={`Size breakdown of ${root.name}`}
		>
			{slices.map((s, i) => (
				<path
					key={`${s.path}-${s.ring}-${i}`}
					d={arcPath(
						c,
						c,
						s.ring === 0 ? r0 : r1 + 1,
						s.ring === 0 ? r1 : r2,
						s.start,
						s.end,
					)}
					fill={s.color}
					opacity={s.ring === 0 ? 0.9 : 0.55}
					className={
						s.isDir && onSelect
							? "cursor-pointer transition-opacity hover:opacity-100"
							: ""
					}
					onClick={() => s.isDir && onSelect?.(s.path, s.isDir)}
					onMouseEnter={() => onHover?.({ label: s.label, size: s.size })}
					onMouseLeave={() => onHover?.(null)}
				>
					<title>{s.label}</title>
				</path>
			))}
		</svg>
	);
}
