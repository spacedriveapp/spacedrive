// @ts-nocheck
import type { CloudServiceType } from "./generated/types";
import DriveAmazonS3 from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveGoogleDrive from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveDropbox from "@sd/assets/icons/Drive-Dropbox.png";
import DriveOneDrive from "@sd/assets/icons/Drive-OneDrive.png";
import DriveBackBlaze from "@sd/assets/icons/Drive-BackBlaze.png";
import DrivePCloud from "@sd/assets/icons/Drive-PCloud.png";
import DriveBox from "@sd/assets/icons/Drive-Box.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import DriveIcon from "@sd/assets/icons/Drive.png";

export type VolumeIcon = string;

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

const CLOUD_SCHEMES: readonly CloudServiceType[] = [
	"s3",
	"gdrive",
	"dropbox",
	"onedrive",
	"gcs",
	"azblob",
	"b2",
	"wasabi",
	"spaces",
	"cloud",
];

/** Extracts a cloud service scheme from a mount point like `onedrive://drive-id`. */
export function parseCloudService(mountPoint: string | null): CloudServiceType | null {
	if (!mountPoint) return null;
	const match = mountPoint.match(/^(\w+):\/\//);
	if (!match) return null;
	const scheme = match[1];
	return CLOUD_SCHEMES.includes(scheme as CloudServiceType) ? (scheme as CloudServiceType) : null;
}

/** Returns the icon for a volume from its mount_point scheme, not its display
 *  name, so renamed volumes keep their correct icon. */
export function getVolumeIcon(volume: {
	mount_point: string | null;
	volume_type?: unknown;
}): VolumeIcon {
	const cloudService = parseCloudService(volume.mount_point);
	if (cloudService) return cloudProviderIcons[cloudService] ?? DriveIcon;
	if (volume.volume_type === "External" || volume.volume_type === "Removable") return HDDIcon;
	return DriveIcon;
}
