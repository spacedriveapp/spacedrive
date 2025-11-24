import { CaretRight } from "@phosphor-icons/react";
import clsx from "clsx";
import { useNavigate } from "react-router-dom";
import { useNormalizedCache } from "@sd/ts-client";
import { SpaceItem } from "./SpaceItem";
import type { VolumeItem, CloudServiceType } from "@sd/ts-client";

// Import cloud provider icons
import DriveAmazonS3 from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveGoogleDrive from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveDropbox from "@sd/assets/icons/Drive-Dropbox.png";
import DriveOneDrive from "@sd/assets/icons/Drive-OneDrive.png";
import DriveBackBlaze from "@sd/assets/icons/Drive-BackBlaze.png";
import DrivePCloud from "@sd/assets/icons/Drive-PCloud.png";
import DriveBox from "@sd/assets/icons/Drive-Box.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import DriveIcon from "@sd/assets/icons/Drive.png";

interface VolumesGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
	/** Filter to show tracked, untracked, or all volumes (default: "All") */
	filter?: "TrackedOnly" | "UntrackedOnly" | "All";
}

// Map cloud service types to icons
const cloudProviderIcons: Record<CloudServiceType, string> = {
	s3: DriveAmazonS3,
	gdrive: DriveGoogleDrive,
	dropbox: DriveDropbox,
	onedrive: DriveOneDrive,
	gcs: DriveGoogleDrive,
	azblob: DriveBox,
	b2: DriveBackBlaze,
	wasabi: DriveAmazonS3,
	spaces: DriveAmazonS3,
	cloud: DrivePCloud,
};

// Helper to parse cloud service from volume fingerprint
function parseCloudService(volume: VolumeItem): CloudServiceType | null {
	// Check if this is a cloud volume by looking at mount_point pattern
	const mountPoint = volume.mount_point;
	if (!mountPoint) return null;

	// Parse mount_point for cloud service (format: "s3://bucket-name")
	const match = mountPoint.match(/^(\w+):\/\//);
	if (!match) return null;

	const scheme = match[1];

	// Verify it's a cloud scheme (not file:// or other local schemes)
	if (scheme === "s3" || scheme === "gdrive" || scheme === "dropbox" ||
	    scheme === "onedrive" || scheme === "gcs" || scheme === "azblob" ||
	    scheme === "b2" || scheme === "wasabi" || scheme === "spaces" ||
	    scheme === "cloud") {
		return scheme as CloudServiceType;
	}

	return null;
}

// Get icon for a volume based on its type
function getVolumeIcon(volume: VolumeItem): string {
	// Check if it's a cloud volume (by mount_point pattern or filesystem type)
	const cloudService = parseCloudService(volume);
	if (cloudService) {
		return cloudProviderIcons[cloudService] || DriveIcon;
	}

	// For external drives, use HDD icon
	if (volume.volume_type === "External") {
		return HDDIcon;
	}

	// Default to generic drive icon
	return DriveIcon;
}

export function VolumesGroup({
	isCollapsed,
	onToggle,
	filter = "All",
}: VolumesGroupProps) {
	const navigate = useNavigate();

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
				className="mb-1 flex w-full cursor-default items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wider text-sidebar-ink-faint hover:text-sidebar-ink"
			>
				<CaretRight
					className={clsx("transition-transform", !isCollapsed && "rotate-90")}
					size={10}
					weight="bold"
				/>
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
								volumeData={{
									device_slug: volume.device_slug,
									mount_path: volume.mount_point || "/",
								}}
								rightComponent={getVolumeBadges(volume)}
								customIcon={getVolumeIcon(volume)}
							/>
						))
					)}
				</div>
			)}
		</div>
	);
}
