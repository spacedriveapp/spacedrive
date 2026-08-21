import {
	Section,
	SectionRows,
	SettingsNote,
	SettingsPageContent,
	SettingsSkeleton,
	ToggleSettingItem,
} from "../Layout";
import { useAppConfig } from "../useConfig";

export function PrivacySettings() {
	const { config, isLoading, update } = useAppConfig();

	if (isLoading || !config) return <SettingsSkeleton />;

	return (
		<SettingsPageContent>
			<Section
				title="Telemetry"
				description="Control what Spacedrive shares about how it is used."
			>
				<SectionRows>
					<ToggleSettingItem
						label="Anonymous Usage Data"
						description="Help improve Spacedrive by sharing anonymous usage data"
						value={config.telemetry_enabled}
						onChange={(telemetry_enabled) => update({ telemetry_enabled })}
					/>
				</SectionRows>
				<SettingsNote>
					We collect anonymous usage statistics to understand how Spacedrive is
					used and to prioritize features. No personal data or file contents are
					ever collected.{" "}
					<a
						href="https://spacedrive.com/privacy"
						target="_blank"
						rel="noopener noreferrer"
						className="text-accent hover:underline"
					>
						Read our Privacy Policy
					</a>
				</SettingsNote>
			</Section>
		</SettingsPageContent>
	);
}
