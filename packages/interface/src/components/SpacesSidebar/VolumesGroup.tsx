import { CaretRight } from "@phosphor-icons/react";
import clsx from "clsx";
import { useNormalizedCache } from "@sd/ts-client";
import { SpaceItem } from "./SpaceItem";
import type { VolumeItem } from "@sd/ts-client/generated/types";

interface VolumesGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
	/** Filter to show tracked, untracked, or all volumes (default: "All") */
	filter?: "TrackedOnly" | "UntrackedOnly" | "All";
}

export function VolumesGroup({
	isCollapsed,
	onToggle,
	filter = "All",
}: VolumesGroupProps) {
	const { data: volumesData } = useNormalizedCache({
		wireMethod: "query:volumes.list",
		input: { filter },
		resourceType: "volume",
	});

	const volumes = volumesData?.volumes || [];

	// Helper to render volume badges
	const getVolumeBadges = (volume: VolumeItem) => (
		<>
			{!volume.is_online && (
				<span className="text-xs text-ink-faint">Offline</span>
			)}
			{!volume.is_tracked && (
				<span className="text-xs text-accent">Untracked</span>
			)}
		</>
	);

	return (
		<div>
			{/* Group Header */}
			<button
				onClick={onToggle}
				className="mb-1 flex w-full items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wider text-sidebar-ink-faint hover:text-sidebar-ink"
			>
				<div
					className={clsx(
						"transition-transform",
						!isCollapsed && "rotate-90",
					)}
				>
					<CaretRight size={10} weight="bold" />
				</div>
				<span>Volumes</span>
			</button>

			{/* Volumes List */}
			{!isCollapsed && (
				<div className="space-y-0.5">
					{volumes.length === 0 ? (
						<div className="px-2 py-1 text-xs text-ink-faint">
							No volumes
						</div>
					) : (
						volumes.map((volume) => (
							<SpaceItem
								key={volume.id}
								item={
									{
										id: volume.id,
										item_type: {
											Volume: {
												volume_id: volume.id,
												name: volume.name,
											},
										},
									} as any
								}
								rightComponent={getVolumeBadges(volume)}
								className={
									volume.is_tracked
										? "text-sidebar-inkDull hover:text-sidebar-ink hover:bg-sidebar-selected transition-colors"
										: "text-sidebar-ink-faint hover:text-sidebar-inkDull hover:bg-sidebar-box transition-colors"
								}
								iconWeight={
									volume.is_tracked ? "bold" : "regular"
								}
							/>
						))
					)}
				</div>
			)}
		</div>
	);
}
