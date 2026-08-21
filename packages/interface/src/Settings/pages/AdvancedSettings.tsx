import {
	Section,
	SectionRows,
	SelectSettingItem,
	SettingsNote,
	SettingsPageContent,
	SettingsSkeleton,
	ToggleSettingItem,
} from "../Layout";
import { useAppConfig } from "../useConfig";

const LOG_LEVELS = [
	{ value: "trace", label: "Trace" },
	{ value: "debug", label: "Debug" },
	{ value: "info", label: "Info" },
	{ value: "warn", label: "Warn" },
	{ value: "error", label: "Error" },
];

export function AdvancedSettings() {
	const { config, isLoading, update } = useAppConfig();

	if (isLoading || !config) return <SettingsSkeleton />;

	return (
		<SettingsPageContent>
			<Section
				title="Diagnostics"
				description="Expert options. Incorrect configuration may affect performance."
			>
				<SectionRows>
					<SelectSettingItem
						label="Daemon Log Level"
						description="Set the verbosity of daemon logs"
						value={config.log_level}
						onChange={(log_level) => update({ log_level })}
						options={LOG_LEVELS}
					/>
				</SectionRows>
			</Section>

			<Section
				title="Job Logging"
				description="Detailed records of background jobs."
			>
				<SectionRows>
					<ToggleSettingItem
						label="Enable Job Logging"
						description="Write detailed logs for background jobs"
						value={config.job_logging.enabled}
						onChange={(job_logging_enabled) => update({ job_logging_enabled })}
					/>
					<ToggleSettingItem
						label="Include Debug Logs"
						description="Include verbose debug information in job logs"
						value={config.job_logging.include_debug}
						disabled={!config.job_logging.enabled}
						onChange={(job_logging_include_debug) =>
							update({ job_logging_include_debug })
						}
					/>
				</SectionRows>
				<SettingsNote>
					Job logs are stored in the library's logs directory. Enabling debug
					logs will significantly increase log file sizes.
				</SettingsNote>
			</Section>
		</SettingsPageContent>
	);
}
