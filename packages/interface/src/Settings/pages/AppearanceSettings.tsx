import { useForm } from "react-hook-form";
import { useCoreQuery, useCoreMutation } from "../../contexts/SpacedriveContext";

interface AppearanceSettingsForm {
  theme: string;
  language: string;
}

export function AppearanceSettings() {
  const { data: config, refetch } = useCoreQuery({ type: "config.app.get", input: null as any });
  const updateConfig = useCoreMutation("config.app.update");

  const form = useForm<AppearanceSettingsForm>({
    values: {
      theme: config?.preferences?.theme || "system",
      language: config?.preferences?.language || "en",
    },
  });

  const onSubmit = form.handleSubmit(async (data) => {
    await updateConfig.mutateAsync({
      theme: data.theme,
      language: data.language,
    });
    refetch();
  });

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">Appearance</h2>
        <p className="text-sm text-ink-dull">
          Customize how Spacedrive looks.
        </p>
      </div>

      <form onSubmit={onSubmit} className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <label className="block">
            <span className="text-sm font-medium text-ink mb-1 block">Theme</span>
            <p className="text-xs text-ink-dull mb-2">Choose your preferred color theme</p>
            <select
              {...form.register("theme")}
              className="w-full px-3 py-2 bg-app border border-app-line rounded-md text-ink text-sm focus:outline-none focus:ring-2 focus:ring-accent"
            >
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
        </div>

        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <label className="block">
            <span className="text-sm font-medium text-ink mb-1 block">Language</span>
            <p className="text-xs text-ink-dull mb-2">Select your preferred language</p>
            <select
              {...form.register("language")}
              className="w-full px-3 py-2 bg-app border border-app-line rounded-md text-ink text-sm focus:outline-none focus:ring-2 focus:ring-accent"
            >
              <option value="en">English</option>
              <option value="de">Deutsch</option>
              <option value="es">Español</option>
              <option value="fr">Français</option>
              <option value="it">Italiano</option>
              <option value="ja">日本語</option>
              <option value="ko">한국어</option>
              <option value="pt">Português</option>
              <option value="ru">Русский</option>
              <option value="zh">中文</option>
            </select>
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
