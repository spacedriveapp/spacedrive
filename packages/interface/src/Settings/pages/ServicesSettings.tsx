import { useState } from "react";
import {
	Section,
	SectionRows,
	SettingsNote,
	SettingsPageContent,
	SettingsSkeleton,
	ToggleSettingItem,
} from "../Layout";
import { useAppConfig } from "../useConfig";

export function ServicesSettings() {
	const { config, isLoading, update } = useAppConfig();
	const [requiresRestart, setRequiresRestart] = useState(false);

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
