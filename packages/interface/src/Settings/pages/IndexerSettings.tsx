import {
	Section,
	SectionRows,
	SettingsEmptyState,
	SettingsPageContent,
	SettingsSkeleton,
	ToggleSettingItem,
} from "../Layout";
import { useLibraryConfig } from "../useConfig";

export function IndexerSettings() {
	const { libraryId, config, isLoading, update } = useLibraryConfig();

	if (!libraryId) {
		return (
			<SettingsPageContent>
				<Section
					title="Indexer"
					description="Configure what files are indexed in your library."
				>
					<SettingsEmptyState
						title="No library selected"
						description="Open or create a library to configure its indexer."
					/>
				</Section>
			</SettingsPageContent>
		);
	}

	if (isLoading || !config) return <SettingsSkeleton />;

	const indexer = config.indexer;

	return (
		<SettingsPageContent>
			<Section
				title="Exclusions"
				description="Paths the indexer walks past without recording."
			>
				<SectionRows>
					<ToggleSettingItem
						label="Skip System Files"
						description="Ignore OS system files and directories"
						value={indexer.no_system_files}
						onChange={(no_system_files) => update({ no_system_files })}
					/>
					<ToggleSettingItem
						label="Skip Git Repositories"
						description="Ignore .git directories"
						value={indexer.no_git}
						onChange={(no_git) => update({ no_git })}
					/>
					<ToggleSettingItem
						label="Skip Dev Directories"
						description="Ignore node_modules, vendor, target, etc."
						value={indexer.no_dev_dirs}
						onChange={(no_dev_dirs) => update({ no_dev_dirs })}
					/>
					<ToggleSettingItem
						label="Skip Hidden Files"
						description="Ignore files starting with a dot"
						value={indexer.no_hidden}
						onChange={(no_hidden) => update({ no_hidden })}
					/>
				</SectionRows>
			</Section>

			<Section
				title="Filters"
				description="Narrow what gets indexed within the paths above."
			>
				<SectionRows>
					<ToggleSettingItem
						label="Respect .gitignore"
						description="Honor .gitignore files when indexing"
						value={indexer.gitignore}
						onChange={(gitignore) => update({ gitignore })}
					/>
					<ToggleSettingItem
						label="Only Index Images"
						description="Only index image files (photos, graphics)"
						value={indexer.only_images}
						onChange={(only_images) => update({ only_images })}
					/>
				</SectionRows>
			</Section>
		</SettingsPageContent>
	);
}
