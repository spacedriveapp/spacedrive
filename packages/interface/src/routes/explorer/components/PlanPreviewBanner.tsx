import { Eye, X } from "@phosphor-icons/react";
import { Button } from "@spacedrive/primitives";
import { usePlanPreviewStore } from "../hooks/usePlanPreview";
import { formatBytes } from "../utils";

/**
 * The bar across the top of the explorer while it browses a plan: what the
 * operation is, its counts, and the button that leaves it. Navigation underneath works as
 * it always does; every listing just passes the plan along.
 */
export function PlanPreviewBanner() {
	const preview = usePlanPreviewStore((state) => state.preview);
	const leave = usePlanPreviewStore((state) => state.leave);
	if (!preview) return null;

	const summary = preview.plan.summary;
	const parts: string[] = [];
	if (summary.creates.files > 0) {
		parts.push(`${summary.creates.files} new (${formatBytes(summary.creates.bytes)})`);
	}
	if (summary.replaces.files > 0) parts.push(`${summary.replaces.files} replaced`);
	if (summary.moves.files > 0) parts.push(`${summary.moves.files} moved`);
	if (summary.deletes.files > 0) parts.push(`${summary.deletes.files} deleted`);
	const skipped =
		summary.skips.duplicates_confirmed.files +
		summary.skips.duplicate_candidates.files +
		summary.skips.policy.files;
	if (skipped > 0) parts.push(`${skipped} skipped`);
	if (summary.conflicts > 0) parts.push(`${summary.conflicts} conflicts`);

	return (
		<div className="mx-2 mb-1.5 flex items-center gap-3 rounded-md border border-accent/30 bg-accent/10 px-3 py-1.5 text-xs">
			<Eye className="size-4 flex-shrink-0 text-accent" weight="fill" />
			<div className="min-w-0 flex-1 truncate">
				<span className="font-medium text-ink">Previewing: {preview.label}</span>
				{parts.length > 0 && <span className="text-ink-dull"> · {parts.join(", ")}</span>}
			</div>
			<Button type="button" variant="outline" size="sm" onClick={leave}>
				<X className="mr-1 size-3.5" weight="bold" />
				Exit preview
			</Button>
		</div>
	);
}
