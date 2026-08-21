import { useState } from "react";
import { Button, Input } from "@spacedrive/primitives";
import { useCoreMutation, useCoreQuery } from "../../contexts/SpacedriveContext";
import { usePlatform } from "../../contexts/PlatformContext";
import {
	Section,
	SectionCard,
	SectionRows,
	SettingItem,
	SettingsPageContent,
	SettingsValue,
} from "../Layout";
import { useAppConfig } from "../useConfig";

interface FieldProps {
	label: string;
	description: string;
	value: string;
	placeholder: string;
	mono?: boolean;
	onCommit: (value: string) => void;
}

/** Labelled text field that commits its value on blur. */
function Field({
	label,
	description,
	value,
	placeholder,
	mono,
	onCommit,
}: FieldProps) {
	const [draft, setDraft] = useState(value);
	const [focused, setFocused] = useState(false);

	// While unfocused the field mirrors the server value, so an update from
	// elsewhere is reflected without clobbering an in-progress edit.
	const shown = focused ? draft : value;

	return (
		<label className="block">
			<span className="block text-sm text-ink">{label}</span>
			<p className="mb-2 mt-0.5 text-tiny leading-snug text-ink-faint">
				{description}
			</p>
			<Input
				className="w-full"
				inputElementClassName={mono ? "font-mono" : undefined}
				value={shown}
				placeholder={placeholder}
				onFocus={() => {
					setDraft(value);
					setFocused(true);
				}}
				onChange={(e) => setDraft(e.target.value)}
				onBlur={() => {
					setFocused(false);
					const next = draft.trim();
					if (next && next !== value) onCommit(next);
				}}
			/>
		</label>
	);
}

export function GeneralSettings() {
	const platform = usePlatform();
	const statusQuery = useCoreQuery({ type: "core.status", input: {} });
	const { config } = useAppConfig();
	const updateDevice = useCoreMutation("device.update", {
		onSuccess: () => {
			statusQuery.refetch();
		},
	});
	const resetData = useCoreMutation("core.reset");

	const device = statusQuery.data?.device_info;
	const dataDirectory =
		config?.data_dir ?? statusQuery.data?.system?.data_directory ?? "";

	const handleResetData = () => {
		platform.confirm(
			"This will permanently delete all libraries, settings, and cached data. The app will need to be restarted. Are you sure?",
			(confirmed) => {
				if (!confirmed) return;
				resetData.mutate({ confirm: true });
			},
		);
	};

	return (
		<SettingsPageContent>
			<Section
				title="Device"
				description="How this device identifies itself to others on your network."
			>
				<SectionCard padded>
					<div className="space-y-4">
						<Field
							label="Device Name"
							description="User-friendly name for this device"
							value={device?.name ?? ""}
							placeholder="My Computer"
							onCommit={(name) => updateDevice.mutate({ name })}
						/>
						<Field
							label="Device Slug"
							description="Unique identifier for this device (alphanumeric and hyphens only)"
							value={device?.slug ?? ""}
							placeholder="my-computer"
							mono
							onCommit={(slug) => updateDevice.mutate({ slug })}
						/>
					</div>
				</SectionCard>
			</Section>

			<Section
				title="Storage"
				description="Where Spacedrive keeps its libraries, indexes and caches."
			>
				<SectionCard padded>
					<div className="flex flex-col gap-3">
						<SettingsValue>{dataDirectory || "Loading..."}</SettingsValue>
						{platform.revealFile && (
							<div className="flex justify-end">
								<Button
									variant="gray"
									size="sm"
									disabled={!dataDirectory}
									onClick={() => platform.revealFile?.(dataDirectory)}
								>
									Reveal in Finder
								</Button>
							</div>
						)}
					</div>
				</SectionCard>
			</Section>

			<Section
				title="Reset"
				description="Destructive actions that cannot be undone."
			>
				<SectionRows>
					<SettingItem
						label="Reset All Data"
						description="Permanently delete all libraries, settings and cached data"
					>
						<Button
							variant="colored"
							size="sm"
							disabled={resetData.isPending}
							className="border-status-error bg-status-error"
							onClick={handleResetData}
						>
							{resetData.isPending ? "Resetting..." : "Reset"}
						</Button>
					</SettingItem>
					{(resetData.data || resetData.error) && (
						<div className="px-4 py-3">
							<p className="text-tiny leading-relaxed text-ink-faint">
								{resetData.error
									? resetData.error.message
									: resetData.data?.message}
							</p>
						</div>
					)}
				</SectionRows>
			</Section>
		</SettingsPageContent>
	);
}
