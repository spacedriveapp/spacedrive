import { RSPCError } from '@rspc/client';
import { ChangeEvent, useEffect, useRef, useState } from 'react';
import { Controller, UseFormReturn } from 'react-hook-form';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { CheckBox, Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Input, useZodForm, z } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/components/AlertDialog';
import { Platform, usePlatform } from '~/util/Platform';

const schema = z.object({ path: z.string(), indexer_rules_ids: z.array(z.number()) });

type FormFieldValues<U> = U extends UseFormReturn<infer U> ? U : never;

interface Props extends UseDialogProps {
	path: string;
}

const LOCATION_ERROR_MESSAGES: Record<number, string | undefined> = {
	// \u000A is a line break, It works with css white-space: pre-line
	404: 'Location is already linked to another Library.\u000ADo you want to add it to this Library too?',
	409: 'Location already present.\u000ADo you want to relink it?'
};

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

	const form = useZodForm({
		schema,
		defaultValues: {
			path: props.path,
			indexer_rules_ids: []
		}
	});

	useEffect(() => {
		const subscription = form.watch((_, { name }) => {
			// Clear custom location error when user changes location path
			if (name === 'path') {
				form.clearErrors('root.serverError');
				setExceptionCode(0);
			}
		});
		return () => subscription.unsubscribe();
	}, [form, form.watch]);

	const addLocation = async ({ path, indexer_rules_ids }: FormFieldValues<typeof form>) => {
		try {
			await createLocation.mutateAsync({ path, indexer_rules_ids });
		} catch (err) {
			const error = err instanceof Error ? err : new Error(String(err));

			if ('cause' in error && error.cause instanceof RSPCError) {
				const { code } = error.cause.shape;
				if (code !== 0) {
					/**
					 * TODO: On code 409 (NeedRelink), we should query the backend for
					 * the current location indexer_rules_ids, then update the checkboxes
					 * accordingly. However we don't have the location id at this point.
					 * Maybe backend could return the location id in the error?
					 */

					setExceptionCode(code);
					form.reset({}, { keepValues: true });
					form.setError('root.serverError', {
						type: 'custom',
						message: LOCATION_ERROR_MESSAGES[code] ?? 'Unknown error'
					});

					// Throw error to prevent dialog from closing
					throw error;
				}
			}

			showAlertDialog({
				title: 'Error',
				value: error.message ?? 'Failed to add location'
			});
		}
	};

	const confirmAfterError = async ({ path, indexer_rules_ids }: FormFieldValues<typeof form>) => {
		try {
			switch (exceptionCode) {
				case 409: {
					await relinkLocation.mutateAsync(path);
					// TODO: Update relinked location with new indexer rules
					// await updateLocation.mutateAsync({
					// 	id: locationId,
					// 	name: null,
					// 	hidden: null,
					// 	indexer_rules_ids,
					// 	sync_preview_media: null,
					// 	generate_preview_media: null
					// });
					break;
				}
				case 404: {
					await addLocationToLibrary.mutateAsync({ path, indexer_rules_ids });
					break;
				}
			}
		} catch (err) {
			const error = err instanceof Error ? err : new Error(String(err));
			showAlertDialog({
				title: 'Error',
				value: error.message ?? 'Failed to add location'
			});
		}
	};

	return (
		<Dialog
			{...{ dialog, form }}
			title="New Location"
			description={
				platform.platform === 'web'
					? '"As you are using the browser version of Spacedrive you will (for now) need to specify an absolute URL of a directory local to the remote node."'
					: ''
			}
			onSubmit={form.handleSubmit((values) =>
				exceptionCode === 0 ? addLocation(values) : confirmAfterError(values)
			)}
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
											checked={field.value.includes(rule.id)}
											onChange={(event: ChangeEvent) => {
												const checkBoxRef = event.target as HTMLInputElement;
												const checkBoxValue = Number.parseInt(checkBoxRef.value);
												if (checkBoxRef.checked) {
													field.onChange([...field.value, checkBoxValue]);
												} else {
													field.onChange(
														field.value.filter((fieldValue) => fieldValue !== checkBoxValue)
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
				<span className="mt-6 inline-block w-full whitespace-pre-wrap text-center text-[0.9rem] text-red-400">
					{form.formState.errors.root.serverError.message}
				</span>
			)}
		</Dialog>
	);
};
