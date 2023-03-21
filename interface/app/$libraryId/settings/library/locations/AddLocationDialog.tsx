import { ChangeEvent } from 'react';
import { Controller } from 'react-hook-form';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { CheckBox, Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Input, useZodForm, z } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/components/AlertDialog';
import { usePlatform } from '~/util/Platform';

const schema = z.object({ path: z.string(), indexer_rules_ids: z.array(z.number()) });

interface Props extends UseDialogProps {
	path: string;
}

export const AddLocationDialog = (props: Props) => {
	const dialog = useDialog(props);
	const platform = usePlatform();
	const createLocation = useLibraryMutation('locations.create');
	const indexerRulesList = useLibraryQuery(['locations.indexer_rules.list']);

	const form = useZodForm({
		schema,
		defaultValues: {
			path: props.path,
			indexer_rules_ids: []
		}
	});

	return (
		<Dialog
			{...{ dialog, form }}
			title="New Location"
			description={
				platform.platform === 'web'
					? '"As you are using the browser version of Spacedrive you will (for now) need to specify an absolute URL of a directory local to the remote node."'
					: ''
			}
			onSubmit={form.handleSubmit(async ({ path, indexer_rules_ids }) => {
				try {
					if (platform.platform === 'tauri') createLocation.mutate({ path, indexer_rules_ids });
					else await createLocation.mutateAsync({ path, indexer_rules_ids });
				} catch (err) {
					console.error(err);
					showAlertDialog({
						title: 'Error',
						value: 'Failed to add location'
					});
				}
			})}
			ctaLabel="Add"
		>
			<div className="relative flex flex-col">
				<p className="mt-2 text-[0.9rem] font-bold">Path:</p>
				<Input
					type="text"
					onClick={async () => {
						if (!platform.openDirectoryPickerDialog) return;

						const path = await platform.openDirectoryPickerDialog();
						if (!path) return;
						if (typeof path !== 'string') {
							// TODO: Should support for adding multiple locations simultaneously be added?
							showAlertDialog({
								title: 'Error',
								value: "Can't add multiple locations"
							});
							return;
						}

						form.setValue('path', path);
					}}
					readOnly={platform.platform !== 'web'}
					required
					className="mt-3 w-full grow cursor-pointer"
					{...form.register('path')}
				/>
			</div>

			<div className="relative flex flex-col">
				<p className="mt-6 text-[0.9rem] font-bold">File indexing rules:</p>
				<div className="mt-4 mb-3 grid w-full grid-cols-2 gap-4">
					<Controller
						name="indexer_rules_ids"
						control={form.control}
						render={({ field }) => (
							<>
								{indexerRulesList.data?.map((rule) => (
									<div className="flex" key={rule.id}>
										<CheckBox
											value={rule.id}
											onChange={(event: ChangeEvent) => {
												const ref = event.target as HTMLInputElement;
												if (ref.checked) {
													field.onChange([...field.value, Number.parseInt(ref.value)]);
												} else {
													field.onChange(
														field.value.filter((value) => value !== Number.parseInt(ref.value))
													);
												}
											}}
										/>
										<span className="mr-3 ml-0.5 mt-0.5 text-sm font-bold">{rule.name}</span>
									</div>
								))}
							</>
						)}
					/>
				</div>
			</div>
		</Dialog>
	);
};
