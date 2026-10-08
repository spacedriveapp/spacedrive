import { HardDrive, Lock } from "@phosphor-icons/react";
import { useNavigate } from "react-router-dom";
import type { IndexSourceInfo } from "@sd/ts-client";
import { formatBytes } from "../../routes/explorer/utils";
import {
	availabilityBadge,
	availabilityDescription,
	sourceAvailability,
} from "./sourceAvailability";

/**
 * A filesystem source on the Sources page — same card language as adapter
 * sources, because a drive is a source whose adapter is the indexer. Opens
 * the explorer at the source root; detached, unmounted and locked drives
 * browse read-only from their snapshots and say so on the card.
 */

function lastSeenLabel(lastSeenSecs: number): string {
	const elapsed = Math.max(0, Math.floor(Date.now() / 1000) - lastSeenSecs);
	if (elapsed < 60) return "just now";
	const minutes = Math.floor(elapsed / 60);
	if (minutes < 60) return `${minutes}m ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h ago`;
	return `${Math.floor(hours / 24)}d ago`;
}

export function DriveSourceCard({
	source,
	deviceSlug,
}: {
	source: IndexSourceInfo;
	deviceSlug: string | undefined;
}) {
	const navigate = useNavigate();
	const name =
		source.root.split(/[/\\]/).filter(Boolean).pop() ?? source.root;
	const availability = sourceAvailability(source);
	const badge = availabilityBadge(availability);

	const open = () => {
		if (!deviceSlug) return;
		const sdPath = {
			Physical: { device_slug: deviceSlug, path: source.root },
		};
		navigate(`/explorer?path=${encodeURIComponent(JSON.stringify(sdPath))}`);
	};

	return (
		<button
			onClick={open}
			title={availabilityDescription(availability, undefined) ?? source.root}
			className="border-app-line bg-app-box hover:border-app-line/80 hover:bg-app-hover group relative rounded-lg border p-4 text-left transition-all"
		>
			<div className="mb-3 flex items-center gap-3">
				<div className="bg-app-selected/40 flex size-9 shrink-0 items-center justify-center rounded-lg">
					<HardDrive
						size={20}
						weight="bold"
						className={
							availability === "available"
								? "text-ink-dull"
								: "text-ink-faint"
						}
					/>
				</div>
				<div className="min-w-0 flex-1">
					<h3 className="text-ink truncate text-sm font-medium">
						{name}
					</h3>
					<p className="text-ink-faint text-xs">filesystem</p>
				</div>
				{badge && (
					<span className="bg-app-selected/40 text-ink-faint flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium">
						{availability === "locked" && (
							<Lock size={10} weight="bold" />
						)}
						{badge.toLowerCase()}
					</span>
				)}
			</div>

			<div className="text-ink-faint flex items-center justify-between text-xs">
				<span>
					{source.entry_count != null
						? `${source.entry_count.toLocaleString()} items`
						: "indexing"}
					{source.total_bytes != null &&
						` · ${formatBytes(source.total_bytes)}`}
				</span>
				<span>{lastSeenLabel(source.last_seen_secs)}</span>
			</div>
		</button>
	);
}
