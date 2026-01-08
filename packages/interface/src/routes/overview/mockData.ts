/**
 * Mock data for Overview screen
 *
 * This showcases the final vision with features that don't exist yet.
 * Progressively replace with real queries as features are built.
 */

const GB = 1024 * 1024 * 1024;
const TB = 1024 * GB;

export const mockOverviewData = {
  libraryName: "My Library",

  // Hero stats
  totalStorage: 4.2 * TB,
  usedStorage: 2.7 * TB,
  totalFiles: 125_847,
  locationCount: 12,
  tagCount: 156,

  // Devices
  devices: [
    {
      id: "device-1",
      name: "Jamie's MacBook Pro",
      slug: "jamies-macbook",
      os: "MacOS",
      os_version: "15.0",
      hardware_model: "MacBook Pro (14-inch, M3 Max, 2023)",
      network_addresses: ["192.168.1.100"],
      capabilities: {
        indexing: true,
        p2p: true,
        volume_detection: true,
      },
      is_online: true,
      last_seen_at: new Date().toISOString(),
      sync_enabled: true,
      last_sync_at: new Date().toISOString(),
    },
    {
      id: "device-2",
      name: "Jamie's iPhone 15",
      slug: "jamies-iphone",
      os: "IOs",
      os_version: "18.0",
      hardware_model: "iPhone 15 Pro",
      network_addresses: ["192.168.1.101"],
      capabilities: {
        indexing: false,
        p2p: true,
        volume_detection: false,
      },
      is_online: true,
      last_seen_at: new Date().toISOString(),
      sync_enabled: true,
      last_sync_at: new Date().toISOString(),
    },
    {
      id: "device-3",
      name: "Mac Studio",
      slug: "mac-studio",
      os: "MacOS",
      os_version: "14.5",
      hardware_model: "Mac Studio (M2 Ultra, 2023)",
      network_addresses: ["192.168.1.102"],
      capabilities: {
        indexing: true,
        p2p: true,
        volume_detection: true,
      },
      is_online: false,
      last_seen_at: new Date(
        Date.now() - 2 * 24 * 60 * 60 * 1000
      ).toISOString(),
      sync_enabled: true,
      last_sync_at: new Date(
        Date.now() - 2 * 24 * 60 * 60 * 1000
      ).toISOString(),
    },
  ],

  // Volumes
  volumes: [
    {
      id: "vol-1",
      fingerprint: "abc123",
      device_id: "device-1",
      name: "Macintosh HD",
      mount_point: "/System/Volumes/Data",
      volume_type: "Primary",
      disk_type: "SSD",
      file_system: "APFS",
      total_capacity: 2 * TB,
      available_space: 200 * GB,
      is_mounted: true,
      is_tracked: true,
      read_speed_mbps: 3500,
      write_speed_mbps: 3000,
    },
    {
      id: "vol-2",
      fingerprint: "def456",
      device_id: "device-1",
      name: "External SSD",
      mount_point: "/Volumes/External",
      volume_type: "External",
      disk_type: "SSD",
      file_system: "exFAT",
      total_capacity: 2 * TB,
      available_space: 1.5 * TB,
      is_mounted: true,
      is_tracked: true,
      read_speed_mbps: 1000,
      write_speed_mbps: 900,
    },
    {
      id: "vol-3",
      fingerprint: "ghi789",
      device_id: "device-2",
      name: "iPhone Storage",
      mount_point: "/var/mobile",
      volume_type: "Primary",
      disk_type: "SSD",
      file_system: "APFS",
      total_capacity: 256 * GB,
      available_space: 128 * GB,
      is_mounted: true,
      is_tracked: true,
      read_speed_mbps: null,
      write_speed_mbps: null,
    },
    {
      id: "vol-4",
      fingerprint: "cloud-s3",
      device_id: "device-1",
      name: "S3: spacedrive-backups",
      mount_point: "s3://spacedrive-backups",
      volume_type: "Cloud",
      disk_type: "Network",
      file_system: "S3",
      total_capacity: 1 * TB,
      available_space: 872 * GB,
      is_mounted: true,
      is_tracked: true,
      read_speed_mbps: null,
      write_speed_mbps: null,
    },
  ],

  // Projects (mocked smart folder detection)
  projects: [
    {
      id: "proj-1",
      name: "spacedrive",
      path: "/Users/jamie/Projects/spacedrive",
    },
    {
      id: "proj-2",
      name: "website",
      path: "/Users/jamie/Projects/website",
    },
    {
      id: "proj-3",
      name: "research",
      path: "/Users/jamie/Documents/research",
    },
    {
      id: "proj-4",
      name: "dotfiles",
      path: "/Users/jamie/.dotfiles",
    },
    {
      id: "proj-5",
      name: "machine-learning",
      path: "/Users/jamie/Projects/ml-experiments",
    },
    {
      id: "proj-6",
      name: "design-system",
      path: "/Users/jamie/Projects/design-system",
    },
    {
      id: "proj-7",
      name: "blog",
      path: "/Users/jamie/Projects/blog",
    },
    {
      id: "proj-8",
      name: "notes",
      path: "/Users/jamie/Documents/notes",
    },
  ],
};
