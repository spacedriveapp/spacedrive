import { RSPCError } from '@rspc/client';
import { ChangeEvent, useEffect, useState } from 'react';
import { Controller } from 'react-hook-form';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { CheckBox, Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Input, useZodForm, z } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/components/AlertDialog';
import { Platform, usePlatform } from '~/util/Platform';

const schema = z.object({ path: z.string(), indexer_rules_ids: z.array(z.number()) });

interface Props extends UseDialogProps {
	path: string;
}

export const openDirectoryPickerDialog = async (platform: Platform): Promise<string> => {
	if (!platform.openDirectoryPickerDialog) return '';

	const path = await platform.openDirectoryPickerDialog();
	if (!path) return '';
	if (typeof path !== 'string')
		// TODO: Should support for adding multiple locations simultaneously be added?
		throw new Error('Adding multiple locations simultaneously is not supported');

	return path;
};

export const AddLocationDialog = (props: Props) => {
	const dialog = useDialog(props);
	const platform = usePlatform();
	const [exceptionCode, setExceptionCode] = useState<0 | 404 | 409>(0);
	const createLocation = useLibraryMutation('locations.create');
	const relinkLocation = useLibraryMutation('locations.relink');
	const indexerRulesList = useLibraryQuery(['locations.indexer_rules.list']);
	const addLocationToLibrary = useLibraryMutation('locations.addLibrary');
	const deleteLocationIndexerRule = useLibraryMutation('locations.indexer_rules.delete');

	const form = useZodForm({
		schema,
		defaultValues: {
			path: props.path,
			indexer_rules_ids: []
		}
	});

	useEffect(() => {
		const subscription = form.watch((_, { name }) => {
			if (name === 'path') {
				form.clearErrors('root.serverError');
				setExceptionCode(0);
			}
		});
		return () => subscription.unsubscribe();
	}, [form, form.watch]);

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
				if (exceptionCode === 0) {
					try {
						await createLocation.mutateAsync({ path, indexer_rules_ids });
					} catch (err) {
						const error = err instanceof Error ? err : new Error(String(err));

						if ('cause' in error && error.cause instanceof RSPCError) {
							const { code } = error.cause.shape;
							if (code === 404 || code === 409) {
								setExceptionCode(code);
								form.reset({}, { keepValues: true });
								form.setError('root.serverError', {
									type: 'custom',
									message:
										// \u000A is a line break, It works with css white-space: pre-line
										code === 404
											? 'Location is already linked to another Library.\u000ADo you want to add it to this Library too?'
											: code === 409
											? 'Location already present.\u000ADo you want to relink it?'
											: 'Unknown error'
								});
								throw error;
							}
						}

						showAlertDialog({
							title: 'Error',
							value: error.message ?? 'Failed to add location'
						});
					}
				} else {
					try {
						if (exceptionCode === 404) {
							await addLocationToLibrary.mutateAsync({ path, indexer_rules_ids });
						} else if (exceptionCode === 409) {
							await relinkLocation.mutateAsync(path);
							const deleteAllIndexerRules = indexerRulesList.data?.map((rule) =>
								deleteLocationIndexerRule.mutateAsync(rule.id)
							);
							if (deleteAllIndexerRules) await Promise.all<unknown>(deleteAllIndexerRules);
						}
					} catch (err) {
						const error = err instanceof Error ? err : new Error(String(err));
						showAlertDialog({
							title: 'Error',
							value: error.message ?? 'Failed to add location'
						});
					}
				}
			})}
			ctaLabel="Add"
		>
			<div className="relative flex flex-col">
				<p className="mt-2 text-[0.9rem]">Path:</p>
				<Input
					type="text"
					onClick={() =>
						openDirectoryPickerDialog(platform)
							.then((path) => void (path && form.setValue('path', path)))
							.catch((error) =>
								showAlertDialog({
									title: 'Error',
									value: String(error)
								})
							)
					}
					readOnly={platform.platform !== 'web'}
					required
					className="mt-3 w-full grow cursor-pointer"
					{...form.register('path')}
				/>
			</div>

			<div className="relative flex flex-col">
				<p className="mt-6 text-[0.9rem]">File indexing rules:</p>
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

			{form.formState.errors.root?.serverError && (
				<span className="mt-6 inline-block whitespace-pre-wrap text-[0.9rem] text-red-400">
					{form.formState.errors.root.serverError.message}
				</span>
			)}
		</Dialog>
	);
};
