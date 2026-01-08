import {
  CaretLeft,
  CaretRight,
  Database,
  HardDrive,
  Plus,
} from "@phosphor-icons/react";
import DatabaseIcon from "@sd/assets/icons/Database.png";
import DriveIcon from "@sd/assets/icons/Drive.png";
import DriveAmazonS3Icon from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveDropboxIcon from "@sd/assets/icons/Drive-Dropbox.png";
import DriveGoogleDriveIcon from "@sd/assets/icons/Drive-GoogleDrive.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import LocationIcon from "@sd/assets/icons/Location.png";
import ServerIcon from "@sd/assets/icons/Server.png";
import type {
  Device,
  JobListItem,
  ListLibraryDevicesInput,
  Location,
  LocationsListOutput,
  LocationsListQueryInput,
  VolumeItem,
  VolumeListOutput,
  VolumeListQueryInput,
} from "@sd/ts-client";
import { TopBarButton } from "@sd/ui";
import clsx from "clsx";
import { motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";
import Masonry from "react-masonry-css";
import { JobCard } from "../../components/JobManager/components/JobCard";
import { useJobs } from "../../components/JobManager/hooks/useJobs";
import {
  getDeviceIcon,
  useCoreQuery,
  useLibraryMutation,
  useNormalizedQuery,
} from "../../contexts/SpacedriveContext";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

function getVolumeIcon(volumeType: any, name?: string): string {
  // Convert volume type to string if it's an enum variant object
  const volumeTypeStr =
    typeof volumeType === "string"
      ? volumeType
      : volumeType?.Other || JSON.stringify(volumeType);

  // Check for cloud providers by name
  if (name?.includes("S3")) return DriveAmazonS3Icon;
  if (name?.includes("Google")) return DriveGoogleDriveIcon;
  if (name?.includes("Dropbox")) return DriveDropboxIcon;

  // By type
  if (volumeTypeStr === "Cloud") return DriveIcon;
  if (volumeTypeStr === "Network") return ServerIcon;
  if (volumeTypeStr === "Virtual") return DatabaseIcon;
  return HDDIcon;
}

function getDiskTypeLabel(diskType: string): string {
  return diskType === "SSD" ? "SSD" : diskType === "HDD" ? "HDD" : diskType;
}

interface DevicePanelProps {
  onLocationSelect?: (location: Location | null) => void;
}

export function DevicePanel({ onLocationSelect }: DevicePanelProps = {}) {
  const [selectedLocationId, setSelectedLocationId] = useState<string | null>(
    null
  );

  // Fetch all volumes using normalized cache
  const { data: volumesData, isLoading: volumesLoading } = useNormalizedQuery<
    VolumeListQueryInput,
    VolumeListOutput
  >({
    wireMethod: "query:volumes.list",
    input: { filter: "All" },
    resourceType: "volume",
  });

  // Fetch all devices using normalized cache
  const { data: devicesData, isLoading: devicesLoading } = useNormalizedQuery<
    ListLibraryDevicesInput,
    Device[]
  >({
    wireMethod: "query:devices.list",
    input: { include_offline: true, include_details: false },
    resourceType: "device",
  });

  // Fetch all locations using normalized cache
  const { data: locationsData, isLoading: locationsLoading } =
    useNormalizedQuery<LocationsListQueryInput, LocationsListOutput>({
      wireMethod: "query:locations.list",
      input: null,
      resourceType: "location",
    });

  // Get all jobs with real-time updates (local jobs)
  const { jobs: localJobs } = useJobs();

  // Get remote device jobs
  // TODO: This should have its own hook like useJobs, this will not work reactively
  const { data: remoteJobsData } = useCoreQuery({
    type: "jobs.remote.all_devices",
    input: {},
  });

  // Merge local and remote jobs
  const allJobs = [
    ...localJobs,
    ...(remoteJobsData?.jobs_by_device
      ? Object.values(remoteJobsData.jobs_by_device)
          .flat()
          .map((remoteJob) => ({
            id: remoteJob.job_id,
            name: remoteJob.job_type,
            device_id: remoteJob.device_id,
            status: remoteJob.status,
            progress: remoteJob.progress || 0,
            action_type: null,
            action_context: null,
          }))
      : []),
  ] as JobListItem[];

  if (volumesLoading || devicesLoading || locationsLoading) {
    return (
      <div className="overflow-hidden rounded-xl border border-app-line bg-app-box">
        <div className="border-app-line border-b px-6 py-4">
          <h2 className="font-semibold text-base text-ink">Storage Volumes</h2>
          <p className="mt-1 text-ink-dull text-sm">Loading volumes...</p>
        </div>
      </div>
    );
  }

  const volumes = volumesData?.volumes || [];
  const devices = devicesData || [];
  const locations = locationsData?.locations || [];

  // Filter to only show user-visible volumes
  const userVisibleVolumes = volumes.filter(
    (volume) => volume.is_user_visible !== false
  );

  // Group volumes by device_id
  const volumesByDevice = userVisibleVolumes.reduce(
    (acc, volume) => {
      const deviceId = volume.device_id;
      if (!acc[deviceId]) {
        acc[deviceId] = [];
      }
      acc[deviceId].push(volume);
      return acc;
    },
    {} as Record<string, VolumeItem[]>
  );

  // Group locations by device slug
  const locationsByDeviceSlug = locations.reduce(
    (acc, location) => {
      // Extract device_slug from sd_path
      if (
        typeof location.sd_path === "object" &&
        "Physical" in location.sd_path
      ) {
        const deviceSlug = location.sd_path.Physical.device_slug;
        if (!acc[deviceSlug]) {
          acc[deviceSlug] = [];
        }
        acc[deviceSlug].push(location);
      }
      return acc;
    },
    {} as Record<string, Location[]>
  );

  // Create device map for quick lookup
  const deviceMap = devices.reduce(
    (acc, device) => {
      acc[device.id] = device;
      return acc;
    },
    {} as Record<string, Device>
  );

  // Group jobs by device_id
  const jobsByDevice = allJobs.reduce(
    (acc, job) => {
      const deviceId = job.device_id;
      if (!acc[deviceId]) {
        acc[deviceId] = [];
      }
      acc[deviceId].push(job);
      return acc;
    },
    {} as Record<string, JobListItem[]>
  );

  const breakpointColumns = {
    default: 3,
    1600: 2,
    1000: 1,
  };

  return (
    <div className="">
      <Masonry
        breakpointCols={breakpointColumns}
        className="-ml-4 flex w-auto"
        columnClassName="pl-4 bg-clip-padding"
      >
        {devices.map((device) => {
          const deviceVolumes = volumesByDevice[device.id] || [];
          const deviceJobs = jobsByDevice[device.id] || [];
          const deviceLocations = locationsByDeviceSlug[device.slug] || [];

          return (
            <DeviceCard
              device={device}
              jobs={deviceJobs}
              key={device.id}
              locations={deviceLocations}
              onLocationSelect={(location) => {
                if (location) {
                  setSelectedLocationId(location.id);
                } else {
                  setSelectedLocationId(null);
                }
                onLocationSelect?.(location);
              }}
              selectedLocationId={selectedLocationId}
              volumes={deviceVolumes}
            />
          );
        })}

        {devices.length === 0 && (
          <div className="overflow-hidden rounded-xl border border-app-line bg-app-box">
            <div className="py-12 text-center text-ink-faint">
              <HardDrive className="mx-auto mb-3 size-12 opacity-20" />
              <p className="text-sm">No devices detected</p>
              <p className="mt-1 text-xs">Pair a device to get started</p>
            </div>
          </div>
        )}
      </Masonry>
    </div>
  );
}

interface DeviceCardProps {
  device?: Device;
  volumes: VolumeItem[];
  jobs: JobListItem[];
  locations: Location[];
  selectedLocationId: string | null;
  onLocationSelect?: (location: Location | null) => void;
}

function DeviceCard({
  device,
  volumes,
  jobs,
  locations,
  selectedLocationId,
  onLocationSelect,
}: DeviceCardProps) {
  const deviceName = device?.name || "Unknown Device";
  const deviceIconSrc = device ? getDeviceIcon(device) : null;
  const { pause, resume } = useJobs();

  // Format hardware specs
  const cpuInfo = device?.cpu_model
    ? `${device.cpu_model}${device.cpu_physical_cores ? ` � ${device.cpu_physical_cores}C` : ""}`
    : null;
  const ramInfo = device?.memory_total
    ? formatBytes(device.memory_total)
    : null;
  // Convert form_factor enum to string
  const formFactor = device?.form_factor
    ? typeof device.form_factor === "string"
      ? device.form_factor
      : (device.form_factor as any)?.Other || JSON.stringify(device.form_factor)
    : null;
  const manufacturer = device?.manufacturer;

  // Filter active jobs
  const activeJobs = jobs.filter(
    (j) => j.status === "running" || j.status === "paused"
  );

  return (
    <div className="mb-4 overflow-hidden rounded-xl border border-app-line bg-app-darkBox">
      {/* Device Header */}
      <div className="border-app-line border-b bg-app-box px-6 py-4">
        <div className="flex items-center gap-4">
          {/* Left: Device icon and name */}
          <div className="flex min-w-0 flex-1 items-center gap-3">
            {deviceIconSrc ? (
              <img
                alt={deviceName}
                className="size-8 flex-shrink-0 opacity-80"
                src={deviceIconSrc}
              />
            ) : (
              <HardDrive
                className="size-8 flex-shrink-0 text-ink"
                weight="duotone"
              />
            )}
            <div className="min-w-0">
              <h3 className="truncate font-semibold text-base text-ink">
                {deviceName}
              </h3>
              <p className="text-ink-dull text-sm">
                {volumes.length} {volumes.length === 1 ? "volume" : "volumes"}
                {device?.is_online === false && " � Offline"}
              </p>
            </div>
          </div>

          {/* Right: Hardware specs */}
          <div className="flex items-center gap-3 text-ink-dull text-xs">
            {manufacturer && formFactor && (
              <div className="text-right">
                <div className="font-medium text-ink">{manufacturer}</div>
                <div>{formFactor}</div>
              </div>
            )}
            {cpuInfo && (
              <div className="text-right">
                <div
                  className="max-w-[180px] truncate font-medium text-ink"
                  title={cpuInfo}
                >
                  {device?.cpu_model || "CPU"}
                </div>
                <div>
                  {device?.cpu_physical_cores}C / {device?.cpu_cores_logical}T
                </div>
              </div>
            )}
            {ramInfo && (
              <div className="text-right">
                <div className="font-medium text-ink">{ramInfo}</div>
                <div>RAM</div>
              </div>
            )}
          </div>
        </div>
      </div>

      <div>
        {/* Active Jobs Section */}
        {activeJobs.length > 0 && (
          <div className="space-y-2 border-app-line border-b bg-app/50 px-3 py-3">
            {activeJobs.map((job) => (
              <JobCard
                job={job}
                key={job.id}
                onPause={pause}
                onResume={resume}
              />
            ))}
          </div>
        )}

        {/* Locations for this device */}
        {locations.length > 0 && (
          <LocationsScroller
            locations={locations}
            onLocationSelect={onLocationSelect}
            selectedLocationId={selectedLocationId}
          />
        )}

        {/* Volumes for this device */}
        <div className="space-y-3 px-3 py-3">
          {volumes.length > 0 ? (
            volumes.map((volume, idx) => (
              <VolumeBar index={idx} key={volume.id} volume={volume} />
            ))
          ) : (
            <div className="flex flex-col items-center justify-center py-8 text-center">
              <div className="text-ink-faint">
                <HardDrive className="mx-auto mb-2 size-8 opacity-20" />
                <p className="text-xs">No volumes</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

interface LocationsScrollerProps {
  locations: Location[];
  selectedLocationId: string | null;
  onLocationSelect?: (location: Location | null) => void;
}

function LocationsScroller({
  locations,
  selectedLocationId,
  onLocationSelect,
}: LocationsScrollerProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

  const updateScrollState = () => {
    if (!scrollRef.current) return;
    const { scrollLeft, scrollWidth, clientWidth } = scrollRef.current;
    setCanScrollLeft(scrollLeft > 0);
    setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 1);
  };

  useEffect(() => {
    updateScrollState();
    window.addEventListener("resize", updateScrollState);
    return () => window.removeEventListener("resize", updateScrollState);
  }, [locations]);

  const scroll = (direction: "left" | "right") => {
    if (!scrollRef.current) return;
    const scrollAmount = 200;
    scrollRef.current.scrollBy({
      left: direction === "left" ? -scrollAmount : scrollAmount,
      behavior: "smooth",
    });
  };

  return (
    <div className="border-app-line border-b px-3 py-3">
      <div className="relative">
        {/* Left fade and button */}
        {canScrollLeft && (
          <>
            <div className="pointer-events-none absolute top-0 bottom-0 left-0 z-10 w-12 bg-gradient-to-r from-app-darkBox to-transparent" />
            <div className="absolute top-1/2 left-1 z-20 -translate-y-1/2">
              <TopBarButton icon={CaretLeft} onClick={() => scroll("left")} />
            </div>
          </>
        )}

        {/* Scrollable container */}
        <div
          className="scrollbar-hide flex gap-2 overflow-x-auto"
          onScroll={updateScrollState}
          ref={scrollRef}
          style={{ scrollbarWidth: "none" }}
        >
          {locations.map((location) => {
            const isSelected = selectedLocationId === location.id;
            return (
              <button
                className="flex min-w-[80px] flex-shrink-0 flex-col items-center gap-2 rounded-lg p-1 transition-all"
                key={location.id}
                onClick={() => {
                  if (isSelected) {
                    onLocationSelect?.(null);
                  } else {
                    onLocationSelect?.(location);
                  }
                }}
              >
                <div
                  className={clsx(
                    "rounded-lg p-2",
                    isSelected ? "bg-app-box" : "bg-transparent"
                  )}
                >
                  <img
                    alt={location.name}
                    className="size-12 opacity-80"
                    src={LocationIcon}
                  />
                </div>
                <div className="flex w-full flex-col items-center">
                  <div
                    className={clsx(
                      "inline-block max-w-full truncate rounded-md px-2 py-0.5 text-xs",
                      isSelected ? "bg-accent text-white" : "text-ink"
                    )}
                  >
                    {location.name}
                  </div>
                </div>
              </button>
            );
          })}
        </div>

        {/* Right fade and button */}
        {canScrollRight && (
          <>
            <div className="pointer-events-none absolute top-0 right-0 bottom-0 z-10 w-12 bg-gradient-to-l from-app-darkBox to-transparent" />
            <div className="absolute top-1/2 right-1 z-20 -translate-y-1/2">
              <TopBarButton icon={CaretRight} onClick={() => scroll("right")} />
            </div>
          </>
        )}
      </div>
    </div>
  );
}

interface VolumeBarProps {
  volume: VolumeItem;
  index: number;
}

function VolumeBar({ volume, index }: VolumeBarProps) {
  const trackVolume = useLibraryMutation("volumes.track");
  const indexVolume = useLibraryMutation("volumes.index");

  // Get current device to check if this volume is local
  const { data: currentDevice } = useCoreQuery({
    type: "devices.current",
    input: null,
  });

  const handleTrack = async () => {
    try {
      await trackVolume.mutateAsync({
        fingerprint: volume.fingerprint,
      });
    } catch (error) {
      console.error("Failed to track volume:", error);
    }
  };

  const handleIndex = async () => {
    try {
      const result = await indexVolume.mutateAsync({
        fingerprint: volume.fingerprint,
        scope: "Recursive",
      });
      console.log("Volume indexed:", result.message);
    } catch (error) {
      console.error("Failed to index volume:", error);
    }
  };

  if (!volume.total_capacity) {
    return null;
  }

  const totalCapacity = volume.total_capacity;
  const availableBytes = volume.available_capacity || 0;
  const usedBytes = totalCapacity - availableBytes;

  const uniqueBytes = volume.unique_bytes ?? Math.floor(usedBytes * 0.7);
  const duplicateBytes = usedBytes - uniqueBytes;

  const usagePercent = (usedBytes / totalCapacity) * 100;
  const uniquePercent = (uniqueBytes / totalCapacity) * 100;
  const duplicatePercent = (duplicateBytes / totalCapacity) * 100;

  // Convert enum values to strings for safe rendering
  const fileSystem = volume.file_system
    ? typeof volume.file_system === "string"
      ? volume.file_system
      : (volume.file_system as any)?.Other || JSON.stringify(volume.file_system)
    : "Unknown";
  const diskType = volume.disk_type
    ? typeof volume.disk_type === "string"
      ? volume.disk_type
      : (volume.disk_type as any)?.Other || JSON.stringify(volume.disk_type)
    : "Unknown";
  const readSpeed = volume.read_speed_mbps;

  const iconSrc = getVolumeIcon(volume.volume_type, volume.name);
  const volumeTypeStr =
    typeof volume.volume_type === "string"
      ? volume.volume_type
      : (volume.volume_type as any)?.Other ||
        JSON.stringify(volume.volume_type);

  return (
    <motion.div
      animate={{ opacity: 1, y: 0 }}
      className="overflow-hidden rounded-lg border border-app-line/50 bg-app-box"
      initial={{ opacity: 0, y: 10 }}
      transition={{ delay: index * 0.05 }}
    >
      {/* Top row: Info */}
      <div className="flex items-center gap-3 px-3 py-2">
        {/* Icon */}
        <img
          alt={volumeTypeStr}
          className="size-6 flex-shrink-0 opacity-80"
          src={iconSrc}
        />

        {/* Name, actions, and badges */}
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="truncate font-semibold text-ink text-sm">
              {volume.display_name || volume.name}
            </span>
            {!volume.is_online && (
              <span className="rounded border border-app-line bg-app-box px-1.5 py-0.5 text-[10px] text-ink-faint">
                Offline
              </span>
            )}
            {!volume.is_tracked && (
              <button
                className="flex items-center gap-1 rounded border border-accent/20 bg-accent/10 px-1.5 py-0.5 text-[10px] text-accent transition-colors hover:border-accent/30 hover:bg-accent/20 disabled:opacity-50"
                disabled={trackVolume.isPending}
                onClick={handleTrack}
                title="Track this volume"
              >
                <Plus className="size-2.5" weight="bold" />
                {trackVolume.isPending ? "Tracking..." : "Track"}
              </button>
            )}
            {currentDevice && volume.device_id === currentDevice.id && (
              <button
                className="flex items-center gap-1 rounded border border-sidebar-line bg-sidebar-box px-1.5 py-0.5 text-[10px] text-sidebar-ink transition-colors hover:bg-sidebar-selected disabled:opacity-50"
                disabled={indexVolume.isPending}
                onClick={handleIndex}
                title="Index this volume"
              >
                <Database className="size-2.5" weight="bold" />
                {indexVolume.isPending ? "Indexing..." : "Index"}
              </button>
            )}
          </div>

          {/* Badges under name */}
          <div className="flex flex-wrap items-center gap-1.5 text-[10px] text-ink-dull">
            <span className="rounded border border-app-line bg-app-box px-1.5 py-0.5">
              {fileSystem}
            </span>
            <span className="rounded border border-app-line bg-app-box px-1.5 py-0.5">
              {getDiskTypeLabel(diskType)}
            </span>
            <span className="rounded border border-app-line bg-app-box px-1.5 py-0.5">
              {volumeTypeStr}
            </span>
            {volume.total_file_count != null && (
              <span className="rounded border border-accent/20 bg-accent/10 px-1.5 py-0.5 text-accent">
                {volume.total_file_count.toLocaleString()} files
              </span>
            )}
          </div>
        </div>

        {/* Capacity info */}
        <div className="flex-shrink-0 text-right">
          <div className="font-medium text-ink text-sm">
            {formatBytes(totalCapacity)}
          </div>
          <div className="text-[10px] text-ink-dull">
            {formatBytes(availableBytes)} free
          </div>
        </div>
      </div>

      {/* Bottom: Full-width capacity bar with padding */}
      <div className="px-3 pt-2 pb-3">
        <div className="h-8 overflow-hidden rounded-md border border-app-line bg-app">
          <div className="flex h-full">
            <motion.div
              animate={{ width: `${uniquePercent}%` }}
              className="border-accent-deep border-r bg-accent"
              initial={{ width: 0 }}
              title={`Unique: ${formatBytes(uniqueBytes)} (${uniquePercent.toFixed(1)}%)`}
              transition={{
                duration: 1,
                ease: "easeOut",
                delay: index * 0.05,
              }}
            />
            <motion.div
              animate={{ width: `${duplicatePercent}%` }}
              className="bg-accent/60"
              initial={{ width: 0 }}
              style={{
                backgroundImage:
                  "repeating-linear-gradient(45deg, transparent, transparent 4px, rgba(255,255,255,0.1) 4px, rgba(255,255,255,0.1) 8px)",
              }}
              title={`Duplicate: ${formatBytes(duplicateBytes)} (${duplicatePercent.toFixed(1)}%)`}
              transition={{
                duration: 1,
                ease: "easeOut",
                delay: index * 0.05 + 0.2,
              }}
            />
          </div>
        </div>
      </div>
    </motion.div>
  );
}
