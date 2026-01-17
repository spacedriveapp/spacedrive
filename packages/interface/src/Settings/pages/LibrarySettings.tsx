import { useForm } from "react-hook-form";
import { useLibraryQuery, useLibraryMutation, useSpacedriveClient } from "../../contexts/SpacedriveContext";

interface LibrarySettingsForm {
  generate_thumbnails: boolean;
  thumbnail_quality: number;
  enable_ai_tagging: boolean;
  sync_enabled: boolean;
  encryption_enabled: boolean;
  auto_track_system_volumes: boolean;
  auto_track_external_volumes: boolean;
}

export function LibrarySettings() {
  const client = useSpacedriveClient();
  const libraryId = client.getCurrentLibraryId();
  const { data: config, refetch, isLoading } = useLibraryQuery(
    { type: "config.library.get", input: null as any },
    { enabled: !!libraryId }
  );
  const updateConfig = useLibraryMutation("config.library.update");

  const form = useForm<LibrarySettingsForm>({
    values: {
      generate_thumbnails: config?.generate_thumbnails ?? true,
      thumbnail_quality: config?.thumbnail_quality ?? 85,
      enable_ai_tagging: config?.enable_ai_tagging ?? false,
      sync_enabled: config?.sync_enabled ?? false,
      encryption_enabled: config?.encryption_enabled ?? false,
      auto_track_system_volumes: config?.auto_track_system_volumes ?? true,
      auto_track_external_volumes: config?.auto_track_external_volumes ?? false,
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
          <h2 className="text-lg font-semibold text-ink mb-2">Library</h2>
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
          <h2 className="text-lg font-semibold text-ink mb-2">Library</h2>
          <p className="text-sm text-ink-dull">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">Library</h2>
        <p className="text-sm text-ink-dull">
          Configure settings for the current library.
        </p>
      </div>

      <form onSubmit={onSubmit} className="space-y-4">
        {/* Media Section */}
        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <h3 className="text-sm font-medium text-ink">Media</h3>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Generate Thumbnails</span>
              <p className="text-xs text-ink-dull">Create preview images for media files</p>
            </div>
            <input
              type="checkbox"
              {...form.register("generate_thumbnails")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="block">
            <span className="text-sm text-ink mb-1 block">Thumbnail Quality</span>
            <p className="text-xs text-ink-dull mb-2">Quality setting for generated thumbnails (1-100)</p>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min="1"
                max="100"
                {...form.register("thumbnail_quality", { valueAsNumber: true })}
                className="flex-1 h-2 rounded-lg appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-moz-range-thumb]:appearance-none [&::-moz-range-thumb]:w-4 [&::-moz-range-thumb]:h-4 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:bg-accent [&::-moz-range-thumb]:border-0"
                style={{
                  background: `linear-gradient(to right, hsl(var(--color-accent)) 0%, hsl(var(--color-accent)) ${((form.watch("thumbnail_quality") - 1) / 99) * 100}%, hsl(var(--color-app)) ${((form.watch("thumbnail_quality") - 1) / 99) * 100}%, hsl(var(--color-app)) 100%)`
                }}
              />
              <span className="text-sm text-ink w-8">{form.watch("thumbnail_quality")}</span>
            </div>
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">AI Tagging</span>
              <p className="text-xs text-ink-dull">Enable AI-powered automatic tagging</p>
            </div>
            <input
              type="checkbox"
              {...form.register("enable_ai_tagging")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>
        </div>

        {/* Sync & Security Section */}
        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <h3 className="text-sm font-medium text-ink">Sync & Security</h3>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Sync Enabled</span>
              <p className="text-xs text-ink-dull">Sync this library across devices</p>
            </div>
            <input
              type="checkbox"
              {...form.register("sync_enabled")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Encryption</span>
              <p className="text-xs text-ink-dull">Encrypt library data at rest</p>
            </div>
            <input
              type="checkbox"
              {...form.register("encryption_enabled")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>
        </div>

        {/* Auto-Tracking Section */}
        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <h3 className="text-sm font-medium text-ink">Auto-Tracking</h3>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">System Volumes</span>
              <p className="text-xs text-ink-dull">Automatically track system drives</p>
            </div>
            <input
              type="checkbox"
              {...form.register("auto_track_system_volumes")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">External Volumes</span>
              <p className="text-xs text-ink-dull">Automatically track external drives when connected</p>
            </div>
            <input
              type="checkbox"
              {...form.register("auto_track_external_volumes")}
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
