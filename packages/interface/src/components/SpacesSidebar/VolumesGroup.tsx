import { useNavigate } from "react-router-dom";
import { useNormalizedQuery, getVolumeIcon } from "@sd/ts-client";
import { SpaceItem } from "./SpaceItem";
import { GroupHeader } from "./GroupHeader";
import type { VolumeItem } from "@sd/ts-client";

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
	const navigate = useNavigate();

	const { data: volumesData } = useNormalizedQuery({
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
			<GroupHeader label="Volumes" isCollapsed={isCollapsed} onToggle={onToggle} />

			{/* Volumes List */}
			{!isCollapsed && (
				<div className="space-y-0.5">
					{volumes.length === 0 ? (
						<div className="px-2 py-1 text-xs text-ink-faint">
							No volumes
						</div>
					) : (
						volumes.map((volume, index) => (
							<SpaceItem
								key={volume.id}
								item={
									{
										id: volume.id,
										item_type: {
											Volume: {
												volume_id: volume.id,
												name: volume.display_name || volume.name,
											},
										},
									} as any
								}
								volumeData={{
									device_slug: volume.device_slug,
									mount_path: volume.mount_point || "/",
								}}
								rightComponent={getVolumeBadges(volume)}
								customIcon={getVolumeIcon(volume)}
								allowInsertion={false}
								isLastItem={index === volumes.length - 1}
							/>
						))
					)}
				</div>
			)}
		</div>
	);
}
