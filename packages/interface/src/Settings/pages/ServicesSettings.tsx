import { useState } from "react";
import {
	ArrowClockwise,
	CheckCircle,
	CircleNotch,
	WarningCircle,
} from "@phosphor-icons/react";
import type {
	ExternalToolStatus,
	ToolCapability,
	ToolInstaller,
} from "@sd/ts-client";
import { Button } from "@spacedrive/primitives";
import {
	useCoreMutation,
	useCoreQuery,
} from "../../contexts/SpacedriveContext";
import { usePlatform } from "../../contexts/PlatformContext";
import {
	Section,
	SectionCard,
	SectionRows,
	SettingsEmptyState,
	SettingsNote,
	SettingsPageContent,
	SettingsSkeleton,
	ToggleSettingItem,
} from "../Layout";
import { useAppConfig } from "../useConfig";

const CAPABILITY_LABELS: Record<ToolCapability, string> = {
	video_thumbnail: "Video thumbnails",
	video_thumbstrip: "Timeline previews",
	media_metadata: "Media metadata",
};

const INSTALL_COMMANDS: Record<ToolInstaller, string> = {
	homebrew: "brew install ffmpeg",
	winget: "winget install --id Gyan.FFmpeg",
};

function ExternalToolCard({
	tool,
	installing,
	installError,
	onInstall,
}: {
	tool: ExternalToolStatus;
	installing: boolean;
	installError: string | null;
	onInstall: (installer: ToolInstaller, label: string) => void;
}) {
	const installers = tool.install_options.filter((option) => option.available);
	const missingProbe =
		tool.id === "ffmpeg" &&
		tool.available &&
		!tool.capabilities.includes("video_thumbstrip");
	const ready = tool.available && !missingProbe;
	const canInstall = !ready && installers.length > 0;

	return (
		<div className="space-y-3 px-4 py-4">
			<div className="flex items-start justify-between gap-4">
				<div className="min-w-0">
					<div className="flex items-center gap-1.5">
						{ready ? (
							<CheckCircle
								className="size-4 shrink-0 text-status-success"
								weight="fill"
							/>
						) : (
							<WarningCircle
								className="size-4 shrink-0 text-status-warning"
								weight="fill"
							/>
						)}
						<p className="text-sm font-medium text-ink">{tool.name}</p>
					</div>
					<p className="mt-0.5 text-tiny text-ink-faint">
						{tool.available
							? tool.version ?? "Available"
							: "Not found on this device"}
					</p>
				</div>

				{canInstall && (
					<div className="flex shrink-0 flex-wrap justify-end gap-2">
						{installers.map((option) => (
							<Button
								key={option.installer}
								variant="gray"
								size="sm"
								disabled={installing}
								onClick={() => onInstall(option.installer, option.label)}
							>
								{installing ? (
									<>
										<CircleNotch className="mr-1.5 size-3.5 animate-spin" />
										Installing...
									</>
								) : (
									option.label
								)}
							</Button>
						))}
					</div>
				)}
			</div>

			{tool.available && (
				<>
					{tool.path && (
						<p
							className="truncate rounded-md bg-app-input px-2.5 py-1.5 font-mono text-tiny text-ink-dull"
							title={tool.path}
						>
							{tool.path}
						</p>
					)}
					<div className="flex flex-wrap gap-1.5">
						{tool.capabilities.map((capability) => (
							<span
								key={capability}
								className="rounded-full bg-app-input px-2 py-1 text-tiny text-ink-dull"
							>
								{CAPABILITY_LABELS[capability]}
							</span>
						))}
					</div>
				</>
			)}

			{!tool.available && installers.length === 0 && (
				<p className="text-tiny leading-relaxed text-ink-faint">
					Install {tool.name} through your system package manager, then check
					again. Spacedrive will discover it without changing the app bundle.
				</p>
			)}
			{missingProbe && (
				<p className="text-tiny leading-relaxed text-status-warning">
					FFprobe was not found beside FFmpeg. Video thumbnails will work, but
					timeline previews and media metadata are unavailable.
				</p>
			)}
			{installError && (
				<p className="text-tiny leading-relaxed text-status-error">
					{installError}
				</p>
			)}
		</div>
	);
}

export function ServicesSettings() {
	const { config, isLoading, update } = useAppConfig();
	const platform = usePlatform();
	const [requiresRestart, setRequiresRestart] = useState(false);
	const toolsQuery = useCoreQuery({ type: "tools.list", input: {} });
	const installTool = useCoreMutation("tools.install", {
		onSuccess: () => toolsQuery.refetch(),
	});

	const install = (
		tool: ExternalToolStatus,
		installer: ToolInstaller,
		label: string,
	) => {
		const command = INSTALL_COMMANDS[installer];
		platform.confirm(
			`${label}? Spacedrive will run “${command}” on this device. The package manager controls the download and installation.`,
			(confirmed) => {
				if (!confirmed) return;
				installTool.reset();
				installTool.mutate({ tool: tool.id, installer, confirm: true });
			},
		);
	};

	if (isLoading || !config) return <SettingsSkeleton />;

	const services = config.services;

	const toggle = (patch: Parameters<typeof update>[0]) => {
		update(patch).then((result) => {
			if (result.requires_restart) setRequiresRestart(true);
		});
	};

	return (
		<SettingsPageContent>
			<Section
				title="External Tools"
				description="Optional software installed on this device that adds media capabilities without increasing the Spacedrive download."
				right={
					<Button
						variant="gray"
						size="sm"
						disabled={toolsQuery.isFetching || installTool.isPending}
						onClick={() => toolsQuery.refetch()}
					>
						<ArrowClockwise
							className={`mr-1.5 size-3.5 ${toolsQuery.isFetching ? "animate-spin" : ""}`}
						/>
						Check again
					</Button>
				}
			>
				{toolsQuery.isLoading ? (
					<SectionCard padded>
						<div className="flex items-center gap-2 text-xs text-ink-faint">
							<CircleNotch className="size-4 animate-spin" />
							Checking this device...
						</div>
					</SectionCard>
				) : toolsQuery.error ? (
					<SettingsNote>
						Spacedrive could not inspect external tools: {toolsQuery.error.message}
					</SettingsNote>
				) : toolsQuery.data?.tools.length ? (
					<SectionCard className="divide-y divide-app-line">
						{toolsQuery.data.tools.map((tool) => (
							<ExternalToolCard
								key={tool.id}
								tool={tool}
								installing={
									installTool.isPending && installTool.variables?.tool === tool.id
								}
								installError={
									installTool.variables?.tool === tool.id
										? installTool.error?.message ?? null
										: null
								}
								onInstall={(installer, label) => install(tool, installer, label)}
							/>
						))}
					</SectionCard>
				) : (
					<SettingsEmptyState
						title="No external tools registered"
						description="This build does not currently use optional host software."
					/>
				)}
			</Section>

			<Section
				title="Background Services"
				description="Daemon subsystems that run while Spacedrive is open."
			>
				<SectionRows>
					<ToggleSettingItem
						label="Networking"
						description="Discover and connect to other devices on your network"
						value={services.networking_enabled}
						onChange={(networking_enabled) => toggle({ networking_enabled })}
					/>
					<ToggleSettingItem
						label="Volume Monitoring"
						description="Detect drives as they are connected and removed"
						value={services.volume_monitoring_enabled}
						onChange={(volume_monitoring_enabled) =>
							toggle({ volume_monitoring_enabled })
						}
					/>
					<ToggleSettingItem
						label="Filesystem Watcher"
						description="Keep indexed locations in sync as files change on disk"
						value={services.fs_watcher_enabled}
						onChange={(fs_watcher_enabled) => toggle({ fs_watcher_enabled })}
					/>
					<ToggleSettingItem
						label="Statistics Listener"
						description="Collect library size and count statistics in the background"
						value={services.statistics_listener_enabled}
						onChange={(statistics_listener_enabled) =>
							toggle({ statistics_listener_enabled })
						}
					/>
				</SectionRows>
				{requiresRestart && (
					<SettingsNote>
						Some of these changes take effect after the daemon restarts.
					</SettingsNote>
				)}
			</Section>
		</SettingsPageContent>
	);
}
