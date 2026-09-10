import {
	Info,
	Gear,
	Briefcase,
	ClockCounterClockwise,
	HardDrive,
	DotsThree,
	Sparkle,
	Image,
	Trash,
	FunnelX,
	ToggleLeft,
	ToggleRight,
	X,
	FilmStrip,
	VideoCamera,
	FolderOpen,
} from "@phosphor-icons/react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate } from "react-router-dom";
import {
	InfoRow,
	Section,
	Divider,
	Tabs,
	TabContent,
} from "../Inspector";
import clsx from "clsx";
import type { Location } from "@sd/ts-client";
import { Button, Dialog, dialogManager, useDialog, CircleButton, type UseDialogProps } from "@spacedrive/primitives";
import { useLibraryMutation } from "../../../contexts/SpacedriveContext";
import LocationIcon from "@sd/assets/icons/Location.webp";

interface LocationInspectorProps {
	location: Location;
}

export function LocationInspector({ location }: LocationInspectorProps) {
	const [activeTab, setActiveTab] = useState("overview");

	const tabs = [
		{ id: "overview", label: "Overview", icon: Info },
		{ id: "activity", label: "Activity", icon: ClockCounterClockwise },
		{ id: "devices", label: "Devices", icon: HardDrive },
		{ id: "more", label: "More", icon: DotsThree },
	];

	return (
		<>
			{/* Tabs */}
			<Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />

			{/* Tab Content */}
			<div className="flex-1 overflow-hidden flex flex-col mt-2.5">
				<TabContent id="overview" activeTab={activeTab}>
					<OverviewTab location={location} />
				</TabContent>

				<TabContent id="activity" activeTab={activeTab}>
					<ActivityTab location={location} />
				</TabContent>

				<TabContent id="devices" activeTab={activeTab}>
					<DevicesTab location={location} />
				</TabContent>

				<TabContent id="more" activeTab={activeTab}>
					<MoreTab location={location} />
				</TabContent>
			</div>
		</>
	);
}

function OverviewTab({ location }: { location: Location }) {
	const routeLocation = useLocation();
	const navigate = useNavigate();
	const isOverview = routeLocation.pathname === '/';

	const formatBytes = (bytes: number | null | undefined) => {
		if (!bytes || bytes === 0) return "0 B";
		const k = 1024;
		const sizes = ["B", "KB", "MB", "GB", "TB"];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
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
			<div className="flex justify-center h-48 items-center w-full px-4">
				<img src={LocationIcon} className="size-24" alt="Location" />
			</div>

			{/* Location name */}
			<div className="px-2 text-center">
				<h4 className="text-sm font-semibold text-sidebar-ink truncate">
					{location.name || "Unnamed Location"}
				</h4>
				<p className="text-xs text-sidebar-inkDull mt-0.5">
					Local Storage
				</p>
			</div>

			<Divider />

			{/* Action Buttons */}
			<div className="px-2 mb-5 flex gap-2">
				{isOverview && (
					<CircleButton
						icon={FolderOpen}
						onClick={() => {
							const encodedPath = encodeURIComponent(JSON.stringify(location.sd_path));
							navigate(`/explorer?path=${encodedPath}`);
						}}
						className="flex-1"
					>
						Open Location
					</CircleButton>
				)}
			</div>

			{/* Details */}
			<Section title="Details" icon={Info}>
				<InfoRow label="Path" value={'Physical' in location.sd_path ? location.sd_path.Physical.path : 'Cloud' in location.sd_path ? location.sd_path.Cloud.path : location.name} mono />
			{location.file_count != null && (
				<InfoRow
					label="Total Files"
					value={location.file_count?.toLocaleString() ?? "0"}
				/>
			)}
				<InfoRow
					label="Total Size"
					value={formatBytes(location.total_size ?? 0)}
				/>
				<InfoRow
					label="Drive"
					value={location.is_available ? "Connected" : "Disconnected"}
				/>
			</Section>
		</div>
	);
}

function ActivityTab({ location: _location }: { location: Location }) {
	const activity = [
		{ action: "Full Scan Completed", time: "10 min ago", files: 12456 },
		{ action: "Thumbnails Generated", time: "1 hour ago", files: 234 },
		{ action: "Content Hashes Updated", time: "3 hours ago", files: 5678 },
		{ action: "Metadata Extracted", time: "5 hours ago", files: 890 },
		{ action: "Location Added", time: "Jan 15, 2025", files: 0 },
	];

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<p className="text-xs text-sidebar-inkDull">
				Recent indexing activity and job history
			</p>

			<div className="space-y-0.5">
				{activity.map((item, i) => (
					<div
						key={i}
						className="flex items-start gap-3 p-2 hover:bg-app-box/40 rounded-lg transition-colors"
					>
						<ClockCounterClockwise
							className="size-4 text-sidebar-inkDull shrink-0 mt-0.5"
							weight="bold"
						/>
						<div className="flex-1 min-w-0">
							<div className="text-xs text-sidebar-ink">
								{item.action}
							</div>
							<div className="text-[11px] text-sidebar-inkDull mt-0.5">
								{item.time}
								{item.files > 0 &&
									` · ${item.files.toLocaleString()} files`}
							</div>
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

function DevicesTab({ location: _location }: { location: Location }) {
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
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<p className="text-xs text-sidebar-inkDull">
				Devices that have access to this location
			</p>

			<div className="space-y-2">
				{devices.map((device, i) => (
					<div
						key={i}
						className="p-2.5 bg-app-box/40 rounded-lg border border-app-line/50"
					>
						<div className="flex items-center gap-2">
							<HardDrive
								className="size-4 text-accent"
								weight="bold"
							/>
							<div className="flex-1 min-w-0">
								<div className="text-xs font-medium text-sidebar-ink">
									{device.name}
								</div>
								<div className="text-[11px] text-sidebar-inkDull flex items-center gap-1">
									<div
										className={clsx(
											"size-1.5 rounded-full",
											device.status === "online"
												? "bg-green-500"
												: "bg-sidebar-inkDull",
										)}
									/>
									<span>
										{device.status === "online"
											? "Online"
											: "Offline"}{" "}
										· {device.lastSeen}
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
	locationId: string;
	locationName: string;
}

function useDeleteLocationDialog() {
	return (locationId: string, locationName: string) =>
		dialogManager.create((props: UseDialogProps) => (
			<DeleteLocationDialog {...props} locationId={locationId} locationName={locationName} />
		));
}

function DeleteLocationDialog({ locationId, locationName, ...props }: DeleteLocationDialogProps) {
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
			dialog={dialog}
			form={form}
			title="Remove Location"
			description={`Are you sure you want to remove "${locationName}"? Your files will not be deleted from disk.`}
			icon={<Trash className="text-red-400" weight="bold" />}
			ctaLabel="Remove Location"
			ctaDanger
			cancelLabel="Cancel"
			cancelBtn
			onSubmit={form.handleSubmit(handleDelete)}
			loading={removeLocation.isPending}
		/>
	);
}

function MoreTab({ location }: { location: Location }) {
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
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<Section title="Advanced" icon={Gear}>
				<InfoRow
					label="Location ID"
					value={String(location.id).slice(0, 8) + "..."}
					mono
				/>
				{location.created_at && (
					<InfoRow
						label="Created"
						value={formatDate(location.created_at)}
					/>
				)}
			</Section>

			<Section title="Danger Zone" icon={Trash}>
				<p className="text-xs text-sidebar-inkDull mb-3">
					Removing this location will not delete your files
				</p>
				<button
					onClick={() => openDeleteDialog(location.id, location.name)}
					className="w-full px-3 py-2 bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 rounded-lg text-sm font-medium text-red-400 transition-colors"
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
	value: _value,
	label,
	description,
	checked,
	onChange,
}: RadioOptionProps) {
	return (
		<button
			onClick={onChange}
			className={clsx(
				"w-full p-2.5 rounded-lg border transition-colors text-left",
				checked
					? "bg-accent/10 border-accent/30"
					: "bg-app-box/40 border-app-line/50 hover:bg-app-box/60",
			)}
		>
			<div className="flex items-start gap-2">
				<div
					className={clsx(
						"size-4 rounded-full border-2 shrink-0 mt-0.5 flex items-center justify-center",
						checked ? "border-accent" : "border-sidebar-inkDull",
					)}
				>
					{checked && (
						<div className="size-2 rounded-full bg-accent" />
					)}
				</div>
				<div className="flex-1 min-w-0">
					<div className="text-xs font-medium text-sidebar-ink">
						{label}
					</div>
					<div className="text-[11px] text-sidebar-inkDull mt-0.5">
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
		<div className="flex items-center gap-2 p-2 bg-app-box/40 rounded-lg border border-app-line/50 group">
			<code className="flex-1 text-xs text-sidebar-ink font-mono">
				{pattern}
			</code>
			<button
				onClick={onRemove}
				className="size-5 rounded flex items-center justify-center opacity-0 group-hover:opacity-100 hover:bg-red-500/20 transition-all"
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
	icon?: React.ComponentType<any>;
}

function JobConfigRow({
	label,
	description,
	enabled,
	onToggle,
	icon: Icon,
}: JobConfigRowProps) {
	return (
		<div className="w-full p-3 bg-app-box/40 rounded-lg border border-app-line/50">
			{/* Header with toggle and icon */}
			<div className="space-y-1.5">
				<button
					onClick={() => onToggle(!enabled)}
					className="flex items-center gap-2.5 w-full text-left group"
				>
					{enabled ? (
						<ToggleRight
							className="size-5 text-accent shrink-0"
							weight="fill"
						/>
					) : (
						<ToggleLeft
							className="size-5 text-sidebar-inkDull shrink-0 group-hover:text-sidebar-ink transition-colors"
							weight="fill"
						/>
					)}
					<div className="flex items-center gap-2 flex-1 min-w-0">
						{Icon && (
							<Icon
								className="size-4 text-sidebar-inkDull shrink-0"
								weight="bold"
							/>
						)}
						<div className="flex-1 min-w-0">
							<div className="text-xs font-medium text-sidebar-ink">
								{label}
							</div>
						</div>
					</div>
				</button>

				{/* Description */}
				<p className="text-[11px] text-sidebar-inkDull leading-relaxed pl-7">
					{description}
				</p>
			</div>

		</div>
	);
}