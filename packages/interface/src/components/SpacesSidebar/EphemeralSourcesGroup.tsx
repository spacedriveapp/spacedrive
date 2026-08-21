import { HardDrive } from "@phosphor-icons/react";
import type { Device, EphemeralSourceInfo } from "@sd/ts-client";
import clsx from "clsx";
import { useLocation, useNavigate } from "react-router-dom";
import {
	useCoreQuery,
	useNormalizedQuery,
} from "../../contexts/SpacedriveContext";
import {
	getSpaceItemKeyFromRoute,
	useExplorer,
} from "../../routes/explorer/context";
import { formatBytes } from "../../routes/explorer/utils";

// Strip trailing separators so "/Volumes/Drive/" and "/Volumes/Drive" compare equal
export function normalizeRoot(path: string): string {
	const trimmed = path.replace(/[/\\]+$/, "");
	return trimmed.length > 0 ? trimmed : path;
}

// Last path component of the source root, e.g. "/Volumes/MyDrive" -> "MyDrive"
function sourceName(root: string): string {
	const parts = root.split(/[/\\]/).filter(Boolean);
	return parts[parts.length - 1] ?? root;
}

function lastSeenLabel(lastSeenSecs: number): string {
	const elapsed = Math.max(0, Math.floor(Date.now() / 1000) - lastSeenSecs);
	if (elapsed < 60) return "just now";
	const minutes = Math.floor(elapsed / 60);
	if (minutes < 60) return `${minutes}m ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h ago`;
	return `${Math.floor(hours / 24)}d ago`;
}

// Indexed drives from the ephemeral cache, including detached ones whose
// restored snapshots remain browsable read-only. Rendered inside the
// Sources group: a drive is a source whose adapter is the filesystem
// indexer, and the sidebar shows that truth even while the storage-side
// registries are still converging.
export function EphemeralSourceRows() {
	const navigate = useNavigate();
	const location = useLocation();
	const { loadPreferencesForSpaceItem } = useExplorer();

	const { data: status } = useCoreQuery({
		type: "core.ephemeral_status",
		input: {},
	});

	const { data: devicesData } = useNormalizedQuery({
		query: "devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	const devices: Device[] = (devicesData as Device[]) ?? [];
	const currentDeviceSlug = devices.find((d) => d.is_current)?.slug;

	// Every indexed filesystem is a source, mounted or not; the Volumes
	// group is about hardware presence, this group is about indexed data.
	const sources = status?.sources ?? [];

	if (sources.length === 0) return null;

	const pathFor = (source: EphemeralSourceInfo): string | null => {
		if (!currentDeviceSlug) return null;
		const sdPath = {
			Physical: { device_slug: currentDeviceSlug, path: source.root },
		};
		return `/explorer?path=${encodeURIComponent(JSON.stringify(sdPath))}`;
	};

	const handleClick = (source: EphemeralSourceInfo) => {
		const path = pathFor(source);
		if (!path) return;
		const [pathname, search] = path.split("?");
		loadPreferencesForSpaceItem(
			getSpaceItemKeyFromRoute(pathname, search ? `?${search}` : ""),
		);
		navigate(path);
	};

	return (
		<div className="space-y-0.5">
					{sources.map((source) => {
						const path = pathFor(source);
						const isActive =
							path !== null &&
							location.pathname + location.search === path;
						return (
							<button
								key={source.id}
								onClick={() => handleClick(source)}
								title={
									source.attached
										? source.root
										: `${source.root} — last seen ${lastSeenLabel(source.last_seen_secs)}`
								}
								className={clsx(
									"flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm font-medium transition-colors",
									isActive
										? "bg-sidebar-selected/30 text-sidebar-ink"
										: source.attached
											? "text-sidebar-inkDull hover:bg-sidebar-selected/20 hover:text-sidebar-ink"
											: "text-ink-faint hover:bg-sidebar-selected/20 hover:text-sidebar-inkDull",
								)}
							>
								<HardDrive
									size={16}
									weight="bold"
									className={clsx(
										"shrink-0",
										!source.attached && "opacity-60",
									)}
								/>
								<span className="flex min-w-0 flex-1 flex-col">
									<span className="truncate">
										{sourceName(source.root)}
									</span>
									{source.entry_count !== null && (
										<span className="text-ink-faint truncate text-[10px] font-normal">
											{source.entry_count.toLocaleString()}{" "}
											items
											{source.total_bytes !== null &&
												` · ${formatBytes(source.total_bytes)}`}
										</span>
									)}
								</span>
								{!source.attached && (
									<span className="bg-sidebar-selected/30 text-ink-faint shrink-0 rounded-full px-1.5 py-px text-[10px] font-medium">
										offline
									</span>
								)}
							</button>
						);
					})}
		</div>
	);
}
