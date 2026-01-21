import { useForm } from "react-hook-form";
import { useCoreQuery, useCoreMutation } from "../../contexts/SpacedriveContext";

interface PrivacySettingsForm {
  telemetry_enabled: boolean;
}

export function PrivacySettings() {
  const { data: config, refetch } = useCoreQuery({ type: "config.app.get", input: null as any });
  const updateConfig = useCoreMutation("config.app.update");

  const form = useForm<PrivacySettingsForm>({
    values: {
      telemetry_enabled: config?.telemetry_enabled ?? true,
    },
  });

  const onSubmit = form.handleSubmit(async (data) => {
    await updateConfig.mutateAsync(data);
    refetch();
  });

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">Privacy</h2>
        <p className="text-sm text-ink-dull">
          Control your privacy and data sharing preferences.
        </p>
      </div>

      <form onSubmit={onSubmit} className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <h3 className="text-sm font-medium text-ink">Telemetry</h3>

          <label className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">Anonymous Usage Data</span>
              <p className="text-xs text-ink-dull">
                Help improve Spacedrive by sharing anonymous usage data
              </p>
            </div>
            <input
              type="checkbox"
              {...form.register("telemetry_enabled")}
              className="h-4 w-4 rounded border-app-line text-accent focus:ring-accent"
            />
          </label>

          <div className="pt-2 border-t border-app-line">
            <p className="text-xs text-ink-dull">
              We collect anonymous usage statistics to understand how Spacedrive is used
              and to prioritize features. No personal data or file contents are ever collected.
            </p>
            <a
              href="https://spacedrive.com/privacy"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-accent hover:underline mt-2 inline-block"
            >
              Read our Privacy Policy
            </a>
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
