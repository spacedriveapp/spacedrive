/* eslint-disable tailwindcss/classnames-order */
import { useLibraryMutation, useLibraryQuery } from "@sd/client";
import { CheckBox, Dialog, UseDialogProps, useDialog } from "@sd/ui";
import { useZodForm, z } from "@sd/ui/src/forms";
import { usePlatform } from "~/util/Platform";
import { Folder } from "~/components/Folder";
import { useEffect, useMemo } from "react";

export interface AddLocationDialog extends UseDialogProps { }

const schema = z.object({
	locations: z.array(z.string())
});

export default function AddRecommendedLocationsDialog({ ...dialogProps }: AddLocationDialog) {
	const platform = usePlatform();

	const { data: recommendedLocations } = useLibraryQuery(['locations.getRecommendedForOs']);
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);


	// This is required because indexRules is undefined on first render
	const indexerRulesIds = useMemo(
		() => listIndexerRules.data?.filter((rule) => rule.default).map((rule) => rule.id) ?? [],
		[listIndexerRules.data]
	);


	const addManyLocations = useLibraryMutation(['locations.addMany']);

	const form = useZodForm({
		schema,
	});

	useEffect(() => {
		if (recommendedLocations) {
			form.setValue('locations', recommendedLocations.map((location) => location.path));
		}
	}, [recommendedLocations]);  // added dependency

	const watchedLocations = form.watch('locations');  // watch the 'locations' field

	const submitHandler = form.handleSubmit(async (values) =>
		addManyLocations.mutate(values.locations));

	const handleChecked = (locationPath: string, checked: boolean) => {
		if (checked) {
			form.setValue('locations', [...watchedLocations, locationPath]);
		} else {
			form.setValue('locations', watchedLocations.filter((path) => path !== locationPath));
		}
	}

	return <Dialog
		form={form}
		title="Add recommended locations for macOS"
		dialog={useDialog(dialogProps)}
		onSubmit={submitHandler}
		ctaLabel="Add"
		description="Locations are places Spacedrive looks for files. You can add and remove locations later."
	>
		<div className="bg-black/20 mt-4 p-4 h-[200px] flex w-full rounded-md overflow-y-scroll gap-2.5 flex-col">
			{recommendedLocations?.map((location) => {
				const checked = watchedLocations?.includes(location.path);
				return (
					<div onClick={(() => handleChecked(location.path, !checked))} key={location.name} className="flex flex-row gap-3 items-center">
						<Folder className="h-8 w-8" />
						<div className="flex flex-col grow">
							<span className="text-sm font-medium">{location.name}</span>
							<span className="text-tiny text-ink-faint/50">{location.path}</span>
						</div>
						<CheckBox onChange={(e) => handleChecked(location.path, e.target.checked)} checked={checked} />
					</div>)

			})}
		</div>
	</Dialog >
}
