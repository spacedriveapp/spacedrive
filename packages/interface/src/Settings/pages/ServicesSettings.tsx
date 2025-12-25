import { useForm } from "react-hook-form";
import { useCoreQuery, useCoreMutation } from "../../context";

interface ServicesSettingsForm {
  networking_enabled: boolean;
  volume_monitoring_enabled: boolean;
  fs_watcher_enabled: boolean;
  statistics_listener_enabled: boolean;
}

export function ServicesSettings() {
  const { data: config, refetch } = useCoreQuery({ type: "config.app.get", input: {} });
  const updateConfig = useCoreMutation("config.app.update");

  const form = useForm<ServicesSettingsForm>({
    values: {
      networking_enabled: config?.services?.networking_enabled ?? true,
      volume_monitoring_enabled: config?.services?.volume_monitoring_enabled ?? true,
      fs_watcher_enabled: config?.services?.fs_watcher_enabled ?? true,
      statistics_listener_enabled: config?.services?.statistics_listener_enabled ?? true,
    },
  });

  const onSubmit = form.handleSubmit(async (data) => {
    const result = await updateConfig.mutateAsync(data);
    refetch();
    
    if (result.requires_restart) {
      alert("Some changes require a daemon restart to take effect.");
    }
  });

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">Services</h2>
        <p className="text-sm text-ink-dull">
          Configure daemon background services.
        </p>
      </div>

      <div className="p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg">
        <p className="text-sm text-amber-400">
          Changes to service settings may require a daemon restart to take effect.
        </p>
      </div>

      <form onSubmit={onSubmit} className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Networking</span>
              <p className="text-xs text-ink-dull">Enable P2P networking and device pairing</p>
            </div>
            <input
              type="checkbox"
              {...form.register("networking_enabled")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Volume Monitoring</span>
              <p className="text-xs text-ink-dull">Monitor for connected and disconnected volumes</p>
            </div>
            <input
              type="checkbox"
              {...form.register("volume_monitoring_enabled")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Filesystem Watcher</span>
              <p className="text-xs text-ink-dull">Watch for file changes in tracked locations</p>
            </div>
            <input
              type="checkbox"
              {...form.register("fs_watcher_enabled")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Statistics Listener</span>
              <p className="text-xs text-ink-dull">Listen for and update library statistics</p>
            </div>
            <input
              type="checkbox"
              {...form.register("statistics_listener_enabled")}
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
