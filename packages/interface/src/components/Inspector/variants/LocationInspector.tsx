import {
  Briefcase,
  ClockCounterClockwise,
  DotsThree,
  FilmStrip,
  FunnelX,
  Gear,
  HardDrive,
  Image,
  Info,
  MagnifyingGlass,
  Play,
  Sparkle,
  ToggleLeft,
  ToggleRight,
  Trash,
  VideoCamera,
  X,
} from "@phosphor-icons/react";
import LocationIcon from "@sd/assets/icons/Location.png";
import type { Location } from "@sd/ts-client";
import {
  Button,
  Dialog,
  dialogManager,
  type UseDialogProps,
  useDialog,
} from "@sd/ui";
import { useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useLibraryMutation } from "../../../contexts/SpacedriveContext";
import { Divider, InfoRow, Section, TabContent, Tabs } from "../Inspector";

interface LocationInspectorProps {
  location: Location;
}

export function LocationInspector({ location }: LocationInspectorProps) {
  const [activeTab, setActiveTab] = useState("overview");

  const tabs = [
    { id: "overview", label: "Overview", icon: Info },
    { id: "indexing", label: "Indexing", icon: Gear },
    { id: "jobs", label: "Jobs", icon: Briefcase },
    { id: "activity", label: "Activity", icon: ClockCounterClockwise },
    { id: "devices", label: "Devices", icon: HardDrive },
    { id: "more", label: "More", icon: DotsThree },
  ];

  return (
    <>
      {/* Tabs */}
      <Tabs activeTab={activeTab} onChange={setActiveTab} tabs={tabs} />

      {/* Tab Content */}
      <div className="mt-2.5 flex flex-1 flex-col overflow-hidden">
        <TabContent activeTab={activeTab} id="overview">
          <OverviewTab location={location} />
        </TabContent>

        <TabContent activeTab={activeTab} id="indexing">
          <IndexingTab location={location} />
        </TabContent>

        <TabContent activeTab={activeTab} id="jobs">
          <JobsTab location={location} />
        </TabContent>

        <TabContent activeTab={activeTab} id="activity">
          <ActivityTab location={location} />
        </TabContent>

        <TabContent activeTab={activeTab} id="devices">
          <DevicesTab location={location} />
        </TabContent>

        <TabContent activeTab={activeTab} id="more">
          <MoreTab location={location} />
        </TabContent>
      </div>
    </>
  );
}

function OverviewTab({ location }: { location: LocationInfo }) {
  const rescanLocation = useLibraryMutation("locations.rescan");

  const formatBytes = (bytes: number | null | undefined) => {
    if (!bytes || bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${Number.parseFloat((bytes / k ** i).toFixed(2))} ${sizes[i]}`;
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const formatScanState = (scanState: any) => {
    if (!scanState) return "Unknown";
    if (scanState.Idle) return "Idle";
    if (scanState.Scanning) return `Scanning ${scanState.Scanning.progress}%`;
    if (scanState.Completed) return "Completed";
    if (scanState.Failed) return "Failed";
    return "Unknown";
  };

  return (
    <div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
      {/* Location icon */}
      <div className="flex h-48 w-full items-center justify-center px-4">
        <img alt="Location" className="size-24" src={LocationIcon} />
      </div>

      {/* Location name */}
      <div className="px-2 text-center">
        <h4 className="truncate font-semibold text-sidebar-ink text-sm">
          {location.name || "Unnamed Location"}
        </h4>
        <p className="mt-0.5 text-sidebar-inkDull text-xs">Local Storage</p>
      </div>

      <Divider />

      {/* Details */}
      <Section icon={Info} title="Details">
        <InfoRow label="Path" mono value={location.path} />
        {location.total_file_count != null && (
          <InfoRow
            label="Total Files"
            value={location.total_file_count?.toLocaleString() ?? "0"}
          />
        )}
        <InfoRow
          label="Total Size"
          value={formatBytes(location.total_byte_size)}
        />
        <InfoRow
          label="Scan State"
          value={formatScanState(location.scan_state)}
        />
        {location.last_scan_at && (
          <InfoRow
            label="Last Scan"
            value={formatDate(location.last_scan_at)}
          />
        )}
      </Section>

      {/* Index Mode */}
      <Section icon={Gear} title="Index Mode">
        <InfoRow
          label="Mode"
          value={
            location.index_mode.charAt(0).toUpperCase() +
            location.index_mode.slice(1)
          }
        />
      </Section>

      {/* Quick Actions */}
      <Section icon={Sparkle} title="Quick Actions">
        <div className="flex flex-col gap-2">
          <button
            className="flex items-center gap-2 rounded-md border border-app-line bg-app-box px-3 py-2 font-medium text-sm transition-colors hover:bg-app-hover disabled:cursor-not-allowed disabled:opacity-50"
            disabled={rescanLocation.isPending}
            onClick={() => {
              rescanLocation.mutate({
                location_id: location.id,
                full_rescan: false,
              });
            }}
          >
            <MagnifyingGlass className="size-4" weight="bold" />
            <span>
              {rescanLocation.isPending ? "Quick Reindex..." : "Quick Reindex"}
            </span>
          </button>
          <button
            className="flex items-center gap-2 rounded-md border border-app-line bg-app-box px-3 py-2 font-medium text-sm transition-colors hover:bg-app-hover disabled:cursor-not-allowed disabled:opacity-50"
            disabled={rescanLocation.isPending}
            onClick={() => {
              rescanLocation.mutate({
                location_id: location.id,
                full_rescan: true,
              });
            }}
          >
            <Sparkle className="size-4" weight="bold" />
            <span>
              {rescanLocation.isPending ? "Full Reindex..." : "Full Reindex"}
            </span>
          </button>
        </div>
      </Section>
    </div>
  );
}

function IndexingTab({ location }: { location: LocationInfo }) {
  const [indexMode, setIndexMode] = useState<"shallow" | "content" | "deep">(
    location.index_mode as "shallow" | "content" | "deep"
  );
  const [ignoreRules, setIgnoreRules] = useState([
    ".git",
    "node_modules",
    "*.tmp",
    ".DS_Store",
  ]);

  return (
    <div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll px-2 pt-2 pb-10">
      <Section icon={Gear} title="Index Mode">
        <p className="mb-3 text-sidebar-inkDull text-xs">
          Controls how deeply this location is indexed
        </p>

        <div className="space-y-2">
          <RadioOption
            checked={indexMode === "shallow"}
            description="Just filesystem metadata (fastest)"
            label="Shallow"
            onChange={() => setIndexMode("shallow")}
            value="shallow"
          />
          <RadioOption
            checked={indexMode === "content"}
            description="Generate content identities"
            label="Content"
            onChange={() => setIndexMode("content")}
            value="content"
          />
          <RadioOption
            checked={indexMode === "deep"}
            description="Full indexing with thumbnails and text extraction"
            label="Deep"
            onChange={() => setIndexMode("deep")}
            value="deep"
          />
        </div>
      </Section>

      <Section icon={FunnelX} title="Ignore Rules">
        <p className="mb-3 text-sidebar-inkDull text-xs">
          Files and folders matching these patterns will be ignored
        </p>

        <div className="space-y-1">
          {ignoreRules.map((pattern, i) => (
            <IgnoreRule
              key={i}
              onRemove={() => {
                setIgnoreRules(ignoreRules.filter((_, idx) => idx !== i));
              }}
              pattern={pattern}
            />
          ))}
        </div>

        <button className="mt-2 text-accent text-xs transition-colors hover:text-accent/80">
          + Add Rule
        </button>
      </Section>
    </div>
  );
}

function JobsTab({ location }: { location: LocationInfo }) {
  const updateLocation = useLibraryMutation("locations.update");
  const triggerJob = useLibraryMutation("locations.triggerJob");

  const updatePolicy = async (
    updates: Partial<typeof location.job_policies>
  ) => {
    await updateLocation.mutateAsync({
      id: location.id,
      job_policies: {
        ...location.job_policies,
        ...updates,
      },
    });
  };

  const thumbnails = location.job_policies?.thumbnail?.enabled ?? true;
  const thumbstrips = location.job_policies?.thumbstrip?.enabled ?? true;
  const proxies = location.job_policies?.proxy?.enabled ?? false;
  const ocr = location.job_policies?.ocr?.enabled ?? false;
  const speech = location.job_policies?.speech_to_text?.enabled ?? false;

  return (
    <div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll px-2 pt-2 pb-10">
      <p className="text-sidebar-inkDull text-xs">
        Configure which processing jobs run automatically for this location
      </p>

      <Section icon={Image} title="Media Processing">
        <div className="space-y-2.5">
          <JobConfigRow
            description="Create preview thumbnails for images and videos"
            enabled={thumbnails}
            isTriggering={triggerJob.isPending}
            label="Generate Thumbnails"
            onToggle={(enabled) =>
              updatePolicy({
                thumbnail: {
                  ...(location.job_policies?.thumbnail ?? {}),
                  enabled,
                },
              })
            }
            onTrigger={() =>
              triggerJob.mutate({
                location_id: location.id,
                job_type: "thumbnail",
                force: false,
              })
            }
          />
          <JobConfigRow
            description="Create video storyboard grids (5×5 grid of frames)"
            enabled={thumbstrips}
            icon={FilmStrip}
            isTriggering={triggerJob.isPending}
            label="Generate Thumbstrips"
            onToggle={(enabled) =>
              updatePolicy({
                thumbstrip: {
                  ...(location.job_policies?.thumbstrip ?? {}),
                  enabled,
                },
              })
            }
            onTrigger={() =>
              triggerJob.mutate({
                location_id: location.id,
                job_type: "thumbstrip",
                force: false,
              })
            }
          />
          <JobConfigRow
            description="Create scrubbing proxies for videos (180p @ 15fps)"
            enabled={proxies}
            icon={VideoCamera}
            isTriggering={triggerJob.isPending}
            label="Generate Proxies"
            onToggle={(enabled) =>
              updatePolicy({
                proxy: {
                  ...(location.job_policies?.proxy ?? {}),
                  enabled,
                },
              })
            }
            onTrigger={() =>
              triggerJob.mutate({
                location_id: location.id,
                job_type: "proxy",
                force: false,
              })
            }
          />
        </div>
      </Section>

      <Section icon={Sparkle} title="AI Processing">
        <div className="space-y-2.5">
          <JobConfigRow
            description="Scan images for text content"
            enabled={ocr}
            isTriggering={triggerJob.isPending}
            label="Extract Text (OCR)"
            onToggle={(enabled) =>
              updatePolicy({
                ocr: { ...(location.job_policies?.ocr ?? {}), enabled },
              })
            }
            onTrigger={() =>
              triggerJob.mutate({
                location_id: location.id,
                job_type: "ocr",
                force: false,
              })
            }
          />
          <JobConfigRow
            description="Transcribe audio and video files"
            enabled={speech}
            isTriggering={triggerJob.isPending}
            label="Speech to Text"
            onToggle={(enabled) =>
              updatePolicy({
                speech_to_text: {
                  ...(location.job_policies?.speech_to_text ?? {}),
                  enabled,
                },
              })
            }
            onTrigger={() =>
              triggerJob.mutate({
                location_id: location.id,
                job_type: "speech_to_text",
                force: false,
              })
            }
          />
        </div>
      </Section>
    </div>
  );
}

function ActivityTab({ location }: { location: LocationInfo }) {
  const activity = [
    { action: "Full Scan Completed", time: "10 min ago", files: 12_456 },
    { action: "Thumbnails Generated", time: "1 hour ago", files: 234 },
    { action: "Content Hashes Updated", time: "3 hours ago", files: 5678 },
    { action: "Metadata Extracted", time: "5 hours ago", files: 890 },
    { action: "Location Added", time: "Jan 15, 2025", files: 0 },
  ];

  return (
    <div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll px-2 pt-2 pb-10">
      <p className="text-sidebar-inkDull text-xs">
        Recent indexing activity and job history
      </p>

      <div className="space-y-0.5">
        {activity.map((item, i) => (
          <div
            className="flex items-start gap-3 rounded-lg p-2 transition-colors hover:bg-app-box/40"
            key={i}
          >
            <ClockCounterClockwise
              className="mt-0.5 size-4 shrink-0 text-sidebar-inkDull"
              weight="bold"
            />
            <div className="min-w-0 flex-1">
              <div className="text-sidebar-ink text-xs">{item.action}</div>
              <div className="mt-0.5 text-[11px] text-sidebar-inkDull">
                {item.time}
                {item.files > 0 && ` · ${item.files.toLocaleString()} files`}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function DevicesTab({ location }: { location: LocationInfo }) {
  const devices = [
    {
      name: "MacBook Pro",
      status: "online" as const,
      lastSeen: "2 min ago",
    },
    {
      name: "Desktop PC",
      status: "offline" as const,
      lastSeen: "2 days ago",
    },
    {
      name: "Home Server",
      status: "online" as const,
      lastSeen: "5 min ago",
    },
  ];

  return (
    <div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll px-2 pt-2 pb-10">
      <p className="text-sidebar-inkDull text-xs">
        Devices that have access to this location
      </p>

      <div className="space-y-2">
        {devices.map((device, i) => (
          <div
            className="rounded-lg border border-app-line/50 bg-app-box/40 p-2.5"
            key={i}
          >
            <div className="flex items-center gap-2">
              <HardDrive className="size-4 text-accent" weight="bold" />
              <div className="min-w-0 flex-1">
                <div className="font-medium text-sidebar-ink text-xs">
                  {device.name}
                </div>
                <div className="flex items-center gap-1 text-[11px] text-sidebar-inkDull">
                  <div
                    className={clsx(
                      "size-1.5 rounded-full",
                      device.status === "online"
                        ? "bg-green-500"
                        : "bg-sidebar-inkDull"
                    )}
                  />
                  <span>
                    {device.status === "online" ? "Online" : "Offline"} ·{" "}
                    {device.lastSeen}
                  </span>
                </div>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

interface DeleteLocationDialogProps extends UseDialogProps {
  locationId: number;
  locationName: string;
}

function useDeleteLocationDialog() {
  return (locationId: number, locationName: string) =>
    dialogManager.create((props: DeleteLocationDialogProps) => (
      <DeleteLocationDialog
        {...props}
        locationId={locationId}
        locationName={locationName}
      />
    ));
}

function DeleteLocationDialog({
  locationId,
  locationName,
  ...props
}: DeleteLocationDialogProps) {
  const dialog = useDialog(props);
  const form = useForm();
  const queryClient = useQueryClient();
  const removeLocation = useLibraryMutation("locations.remove", {
    onSuccess: () => {
      // Manually invalidate the locations query until the backend emits ResourceDeleted events
      // This forces a refetch so the location disappears from the sidebar immediately
      queryClient.invalidateQueries({
        predicate: (query) => {
          const key = query.queryKey;
          return Array.isArray(key) && key[0] === "query:locations.list";
        },
      });

      // Close the dialog
      dialogManager.setState(dialog.id, { open: false });
    },
  });

  const handleDelete = async () => {
    try {
      await removeLocation.mutateAsync({
        location_id: String(locationId),
      });
    } catch (error) {
      console.error("Failed to remove location:", error);
    }
  };

  return (
    <Dialog
      cancelBtn
      cancelLabel="Cancel"
      ctaDanger
      ctaLabel="Remove Location"
      description={`Are you sure you want to remove "${locationName}"? Your files will not be deleted from disk.`}
      dialog={dialog}
      form={form}
      icon={<Trash className="text-red-400" weight="bold" />}
      loading={removeLocation.isPending}
      onSubmit={handleDelete}
      title="Remove Location"
    />
  );
}

function MoreTab({ location }: { location: LocationInfo }) {
  const openDeleteDialog = useDeleteLocationDialog();

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  return (
    <div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll px-2 pt-2 pb-10">
      <Section icon={Gear} title="Advanced">
        <InfoRow
          label="Location ID"
          mono
          value={String(location.id).slice(0, 8) + "..."}
        />
        {location.created_at && (
          <InfoRow label="Created" value={formatDate(location.created_at)} />
        )}
        {location.last_scan_at && (
          <InfoRow
            label="Last Scan"
            value={formatDate(location.last_scan_at)}
          />
        )}
      </Section>

      <Section icon={Trash} title="Danger Zone">
        <p className="mb-3 text-sidebar-inkDull text-xs">
          Removing this location will not delete your files
        </p>
        <button
          className="w-full rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 font-medium text-red-400 text-sm transition-colors hover:bg-red-500/20"
          onClick={() => openDeleteDialog(location.id, location.name)}
        >
          <div className="flex items-center justify-center gap-2">
            <Trash className="size-4" weight="bold" />
            <span>Remove Location</span>
          </div>
        </button>
      </Section>
    </div>
  );
}

// Helper Components

interface RadioOptionProps {
  value: string;
  label: string;
  description: string;
  checked: boolean;
  onChange: () => void;
}

function RadioOption({
  value,
  label,
  description,
  checked,
  onChange,
}: RadioOptionProps) {
  return (
    <button
      className={clsx(
        "w-full rounded-lg border p-2.5 text-left transition-colors",
        checked
          ? "border-accent/30 bg-accent/10"
          : "border-app-line/50 bg-app-box/40 hover:bg-app-box/60"
      )}
      onClick={onChange}
    >
      <div className="flex items-start gap-2">
        <div
          className={clsx(
            "mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border-2",
            checked ? "border-accent" : "border-sidebar-inkDull"
          )}
        >
          {checked && <div className="size-2 rounded-full bg-accent" />}
        </div>
        <div className="min-w-0 flex-1">
          <div className="font-medium text-sidebar-ink text-xs">{label}</div>
          <div className="mt-0.5 text-[11px] text-sidebar-inkDull">
            {description}
          </div>
        </div>
      </div>
    </button>
  );
}

interface IgnoreRuleProps {
  pattern: string;
  onRemove: () => void;
}

function IgnoreRule({ pattern, onRemove }: IgnoreRuleProps) {
  return (
    <div className="group flex items-center gap-2 rounded-lg border border-app-line/50 bg-app-box/40 p-2">
      <code className="flex-1 font-mono text-sidebar-ink text-xs">
        {pattern}
      </code>
      <button
        className="flex size-5 items-center justify-center rounded opacity-0 transition-all hover:bg-red-500/20 group-hover:opacity-100"
        onClick={onRemove}
        title="Remove rule"
      >
        <X className="size-3 text-red-400" weight="bold" />
      </button>
    </div>
  );
}

interface JobConfigRowProps {
  label: string;
  description: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  onTrigger: () => void;
  isTriggering: boolean;
  icon?: React.ComponentType<any>;
}

function JobConfigRow({
  label,
  description,
  enabled,
  onToggle,
  onTrigger,
  isTriggering,
  icon: Icon,
}: JobConfigRowProps) {
  return (
    <div className="w-full rounded-lg border border-app-line/50 bg-app-box/40 p-3">
      {/* Header with toggle and icon */}
      <div className="space-y-1.5">
        <button
          className="group flex w-full items-center gap-2.5 text-left"
          onClick={() => onToggle(!enabled)}
        >
          {enabled ? (
            <ToggleRight
              className="size-5 shrink-0 text-accent"
              weight="fill"
            />
          ) : (
            <ToggleLeft
              className="size-5 shrink-0 text-sidebar-inkDull transition-colors group-hover:text-sidebar-ink"
              weight="fill"
            />
          )}
          <div className="flex min-w-0 flex-1 items-center gap-2">
            {Icon && (
              <Icon
                className="size-4 shrink-0 text-sidebar-inkDull"
                weight="bold"
              />
            )}
            <div className="min-w-0 flex-1">
              <div className="font-medium text-sidebar-ink text-xs">
                {label}
              </div>
            </div>
          </div>
        </button>

        {/* Description */}
        <p className="pl-7 text-[11px] text-sidebar-inkDull leading-relaxed">
          {description}
        </p>
      </div>

      {/* Run button */}
      <Button
        className="mt-2.5 flex w-full items-center justify-center gap-1.5"
        disabled={!enabled || isTriggering}
        onClick={onTrigger}
        size="sm"
        title={enabled ? "Run job now" : "Enable job first"}
        variant="gray"
      >
        <Play className="size-3" weight="fill" />
        {isTriggering ? "Running..." : "Run Now"}
      </Button>
    </div>
  );
}
