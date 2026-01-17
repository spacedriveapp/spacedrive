import { useForm } from "react-hook-form";
import { useCoreQuery, useCoreMutation } from "../../contexts/SpacedriveContext";

interface AdvancedSettingsForm {
  job_logging_enabled: boolean;
  job_logging_include_debug: boolean;
  log_level: string;
}

export function AdvancedSettings() {
  const { data: config, refetch } = useCoreQuery({ type: "config.app.get", input: null as any });
  const updateConfig = useCoreMutation("config.app.update");

  const form = useForm<AdvancedSettingsForm>({
    values: {
      job_logging_enabled: config?.job_logging?.enabled ?? true,
      job_logging_include_debug: config?.job_logging?.include_debug ?? false,
      log_level: config?.log_level || "info",
    },
  });

  const onSubmit = form.handleSubmit(async (data) => {
    await updateConfig.mutateAsync(data);
    refetch();
  });

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">Advanced</h2>
        <p className="text-sm text-ink-dull">
          Advanced configuration options for power users.
        </p>
      </div>

      <div className="p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg">
        <p className="text-sm text-amber-400">
          These settings are for expert users. Incorrect configuration may affect performance.
        </p>
      </div>

      <form onSubmit={onSubmit} className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-3">Daemon Log Level</h3>
          <label className="block">
            <p className="text-xs text-ink-dull mb-2">Set the verbosity of daemon logs</p>
            <select
              {...form.register("log_level")}
              className="w-full px-3 py-2 bg-app border border-app-line rounded-md text-ink text-sm focus:outline-none focus:ring-2 focus:ring-accent"
            >
              <option value="trace">Trace</option>
              <option value="debug">Debug</option>
              <option value="info">Info</option>
              <option value="warn">Warn</option>
              <option value="error">Error</option>
            </select>
          </label>
        </div>

        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <h3 className="text-sm font-medium text-ink">Job Logging</h3>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Enable Job Logging</span>
              <p className="text-xs text-ink-dull">Write detailed logs for background jobs</p>
            </div>
            <input
              type="checkbox"
              {...form.register("job_logging_enabled")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Include Debug Logs</span>
              <p className="text-xs text-ink-dull">Include verbose debug information in job logs</p>
            </div>
            <input
              type="checkbox"
              {...form.register("job_logging_include_debug")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <div className="pt-2 border-t border-app-line">
            <p className="text-xs text-ink-dull">
              Job logs are stored in the library's logs directory. Enabling debug logs
              will significantly increase log file sizes.
            </p>
          </div>
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
