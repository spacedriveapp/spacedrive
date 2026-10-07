import { useState } from "react";
import { Slider } from "@spacedrive/primitives";
import {
	Section,
	SectionRows,
	SelectSettingItem,
	SettingItem,
	SettingsEmptyState,
	SettingsPageContent,
	SettingsSkeleton,
	ToggleSettingItem,
} from "../Layout";
import { useLibraryConfig } from "../useConfig";
import { libraryDefaults } from "../../routes/explorer/components/addToLibrary";

function ThumbnailQualityItem({
	value,
	onCommit,
}: {
	value: number;
	onCommit: (value: number) => void;
}) {
	// Once touched the slider drives itself, so committing a value doesn't
	// snap the handle back while the config query catches up.
	const [local, setLocal] = useState<number | null>(null);
	const shown = local ?? value;

	return (
		<SettingItem
			label="Thumbnail Quality"
			description="Quality setting for generated thumbnails (1-100)"
		>
			<div className="flex w-44 items-center gap-3">
				<Slider
					className="flex-1"
					min={1}
					max={100}
					step={1}
					value={[shown]}
					onValueChange={([next]) => setLocal(next)}
					onValueCommit={([next]) => onCommit(next)}
				/>
				<span className="w-7 text-right text-xs tabular-nums text-ink-dull">
					{shown}
				</span>
			</div>
		</SettingItem>
	);
}

export function LibrarySettings() {
	const { libraryId, config, isLoading, update } = useLibraryConfig();

	if (!libraryId) {
		return (
			<SettingsPageContent>
				<Section
					title="Library"
					description="Configure settings for the current library."
				>
					<SettingsEmptyState
						title="No library selected"
						description="Open or create a library to configure it."
					/>
				</Section>
			</SettingsPageContent>
		);
	}

	if (isLoading || !config) return <SettingsSkeleton />;

	const adding = libraryDefaults(config.adding);

	return (
		<SettingsPageContent>
			<Section
				title="Media"
				description="How Spacedrive previews and understands your files."
			>
				<SectionRows>
					<ToggleSettingItem
						label="Generate Thumbnails"
						description="Create preview images for media files"
						value={config.generate_thumbnails}
						onChange={(generate_thumbnails) => update({ generate_thumbnails })}
					/>
					<ThumbnailQualityItem
						value={config.thumbnail_quality}
						onCommit={(thumbnail_quality) => update({ thumbnail_quality })}
					/>
					<ToggleSettingItem
						label="AI Tagging"
						description="Enable AI-powered automatic tagging"
						value={config.enable_ai_tagging}
						onChange={(enable_ai_tagging) => update({ enable_ai_tagging })}
					/>
				</SectionRows>
			</Section>

			<Section
				title="Sync & Security"
				description="How this library travels between your devices."
			>
				<SectionRows>
					<ToggleSettingItem
						label="Sync Enabled"
						description="Sync this library across devices"
						value={config.sync_enabled}
						onChange={(sync_enabled) => update({ sync_enabled })}
					/>
					<ToggleSettingItem
						label="Encryption"
						description="Encrypt library data at rest"
						value={config.encryption_enabled}
						onChange={(encryption_enabled) => update({ encryption_enabled })}
					/>
				</SectionRows>
			</Section>

			<Section
				title="Adding content"
				description="What Add to Library starts from. Each add can change these for itself; changing them here never moves an existing catalog."
			>
				<SectionRows>
					<SelectSettingItem
						label="Store catalog"
						description="In the library keeps it under the Spacedrive data folder; on source writes it beside the files, in .spacedrive, so it travels with the drive."
						value={adding.placement}
						onChange={(placement) => update({ adding: { placement } })}
						options={[
							{ value: "in_library", label: "In library" },
							{ value: "on_source", label: "On source" },
						]}
					/>
					<ToggleSettingItem
						label="Keep an Offline Copy"
						description="For an on-source catalog, keep a replica in the library so it answers while the drive is unplugged"
						value={adding.keep_offline_copy}
						onChange={(keep_offline_copy) => update({ adding: { keep_offline_copy } })}
					/>
					<ToggleSettingItem
						label="Capture Everything"
						description="Record system files, .git and dev directories instead of skipping them"
						value={adding.unfiltered}
						onChange={(unfiltered) => update({ adding: { unfiltered } })}
					/>
					<ToggleSettingItem
						label="Identify Content"
						description="Hash each file once a walk lands, for duplicates and integrity checks"
						value={adding.identify_content}
						onChange={(identify_content) => update({ adding: { identify_content } })}
					/>
				</SectionRows>
			</Section>

			<Section
				title="Auto-Tracking"
				description="Which volumes are added to this library on sight."
			>
				<SectionRows>
					<ToggleSettingItem
						label="System Volumes"
						description="Automatically track system drives"
						value={config.auto_track_system_volumes}
						onChange={(auto_track_system_volumes) =>
							update({ auto_track_system_volumes })
						}
					/>
					<ToggleSettingItem
						label="External Volumes"
						description="Automatically track external drives when connected"
						value={config.auto_track_external_volumes}
						onChange={(auto_track_external_volumes) =>
							update({ auto_track_external_volumes })
						}
					/>
				</SectionRows>
			</Section>
		</SettingsPageContent>
	);
}
