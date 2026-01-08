// @ts-nocheck

import DriveIcon from "@sd/assets/icons/Drive.png";
import DriveAmazonS3 from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveBackBlaze from "@sd/assets/icons/Drive-BackBlaze.png";
import DriveBox from "@sd/assets/icons/Drive-Box.png";
import DriveDropbox from "@sd/assets/icons/Drive-Dropbox.png";
import DriveGoogleDrive from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveOneDrive from "@sd/assets/icons/Drive-OneDrive.png";
import DrivePCloud from "@sd/assets/icons/Drive-PCloud.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import type { CloudServiceType } from "./generated/types";

export type VolumeIcon = string;

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

/**
 * Parse cloud service type from volume mount point.
 * Cloud volumes typically have mount points like "s3://bucket-name"
 */
function parseCloudService(mountPoint: string | null): CloudServiceType | null {
  if (!mountPoint) return null;

  // Parse mount_point for cloud service (format: "s3://bucket-name")
  const match = mountPoint.match(/^(\w+):\/\//);
  if (!match) return null;

  const scheme = match[1];

  // Verify it's a cloud scheme (not file:// or other local schemes)
  const cloudSchemes: CloudServiceType[] = [
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

  if (cloudSchemes.includes(scheme as CloudServiceType)) {
    return scheme as CloudServiceType;
  }

  return null;
}

/**
 * Determines the appropriate volume icon based on volume information.
 *
 * Priority order:
 * 1. Cloud service type (parsed from mount point)
 * 2. Volume type (External vs Internal)
 * 3. Default to generic drive icon
 */
export function getVolumeIcon(volume: {
  mount_point: string | null;
  volume_type?: "Internal" | "External" | "Removable";
}): VolumeIcon {
  // Check if it's a cloud volume
  const cloudService = parseCloudService(volume.mount_point);
  if (cloudService) {
    return cloudProviderIcons[cloudService] || DriveIcon;
  }

  // For external/removable drives, use HDD icon
  if (volume.volume_type === "External" || volume.volume_type === "Removable") {
    return HDDIcon;
  }

  // Default to generic drive icon
  return DriveIcon;
}
