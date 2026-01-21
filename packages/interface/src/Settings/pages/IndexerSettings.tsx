import { useForm } from "react-hook-form";
import { useLibraryQuery, useLibraryMutation, useSpacedriveClient } from "../../contexts/SpacedriveContext";

interface IndexerSettingsForm {
  no_system_files: boolean;
  no_git: boolean;
  no_dev_dirs: boolean;
  no_hidden: boolean;
  gitignore: boolean;
  only_images: boolean;
}

export function IndexerSettings() {
  const client = useSpacedriveClient();
  const libraryId = client.getCurrentLibraryId();
  const { data: config, refetch, isLoading } = useLibraryQuery(
    { type: "config.library.get", input: null as any },
    { enabled: !!libraryId }
  );
  const updateConfig = useLibraryMutation("config.library.update");

  const form = useForm<IndexerSettingsForm>({
    values: {
      no_system_files: config?.indexer?.no_system_files ?? true,
      no_git: config?.indexer?.no_git ?? true,
      no_dev_dirs: config?.indexer?.no_dev_dirs ?? true,
      no_hidden: config?.indexer?.no_hidden ?? false,
      gitignore: config?.indexer?.gitignore ?? true,
      only_images: config?.indexer?.only_images ?? false,
    },
  });

  const onSubmit = form.handleSubmit(async (data) => {
    await updateConfig.mutateAsync(data);
    refetch();
  });

  if (!libraryId) {
    return (
      <div className="space-y-6">
        <div>
          <h2 className="text-lg font-semibold text-ink mb-2">Indexer</h2>
          <p className="text-sm text-ink-dull">
            No library selected. Please select a library first.
          </p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="space-y-6">
        <div>
          <h2 className="text-lg font-semibold text-ink mb-2">Indexer</h2>
          <p className="text-sm text-ink-dull">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">Indexer</h2>
        <p className="text-sm text-ink-dull">
          Configure what files are indexed in your library.
        </p>
      </div>

      <form onSubmit={onSubmit} className="space-y-4">
        {/* Exclusions Section */}
        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <h3 className="text-sm font-medium text-ink">Exclusions</h3>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Skip System Files</span>
              <p className="text-xs text-ink-dull">Ignore OS system files and directories</p>
            </div>
            <input
              type="checkbox"
              {...form.register("no_system_files")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Skip Git Repositories</span>
              <p className="text-xs text-ink-dull">Ignore .git directories</p>
            </div>
            <input
              type="checkbox"
              {...form.register("no_git")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Skip Dev Directories</span>
              <p className="text-xs text-ink-dull">Ignore node_modules, vendor, target, etc.</p>
            </div>
            <input
              type="checkbox"
              {...form.register("no_dev_dirs")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Skip Hidden Files</span>
              <p className="text-xs text-ink-dull">Ignore files starting with a dot</p>
            </div>
            <input
              type="checkbox"
              {...form.register("no_hidden")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>
        </div>

        {/* Filters Section */}
        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <h3 className="text-sm font-medium text-ink">Filters</h3>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Respect .gitignore</span>
              <p className="text-xs text-ink-dull">Honor .gitignore files when indexing</p>
            </div>
            <input
              type="checkbox"
              {...form.register("gitignore")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Only Index Images</span>
              <p className="text-xs text-ink-dull">Only index image files (photos, graphics)</p>
            </div>
            <input
              type="checkbox"
              {...form.register("only_images")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>
        </div>

        {form.formState.isDirty && (
          <button
            type="submit"
            disabled={updateConfig.isPending}
            className="px-4 py-2 bg-accent hover:bg-accent-deep text-white rounded-md text-sm font-medium transition-colors disabled:opacity-50"
          >
            {updateConfig.isPending ? "Saving..." : "Save Changes"}
          </button>
        )}
      </form>
    </div>
  );
}
