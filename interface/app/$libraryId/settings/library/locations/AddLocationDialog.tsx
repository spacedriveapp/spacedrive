import clsx from 'clsx';
import { useCallback, useEffect, useMemo } from 'react';
import { Controller, get } from 'react-hook-form';
import {
	UnionToTuple,
	extractInfoRSPCError,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { ErrorMessage, Input, useZodForm, z } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/components';
import { useCallbackToWatchForm } from '~/hooks';
import { Platform, usePlatform } from '~/util/Platform';
import IndexerRuleEditor from './IndexerRuleEditor';

const REMOTE_ERROR_FORM_FIELD = 'root.serverError';
const REMOTE_ERROR_FORM_MESSAGE = {
	// \u000A is a line break, It works with css white-space: pre-line
	CREATE: '',
	ADD_LIBRARY:
		'Location is already linked to another Library.\u000ADo you want to add it to this Library too?',
	NEED_RELINK: 'Location already present.\u000ADo you want to relink it?'
};

type RemoteErrorFormMessage = keyof typeof REMOTE_ERROR_FORM_MESSAGE;

const isRemoteErrorFormMessage = (message: unknown): message is RemoteErrorFormMessage =>
	typeof message === 'string' && Object.hasOwnProperty.call(REMOTE_ERROR_FORM_MESSAGE, message);

const schema = z.object({
	path: z.string().min(1),
	method: z.enum(Object.keys(REMOTE_ERROR_FORM_MESSAGE) as UnionToTuple<RemoteErrorFormMessage>),
	indexerRulesIds: z.array(z.number())
});

type SchemaType = z.infer<typeof schema>;

export const openDirectoryPickerDialog = async (platform: Platform): Promise<null | string> => {
	if (!platform.openDirectoryPickerDialog) return null;

	const path = await platform.openDirectoryPickerDialog();
	if (!path) return '';
	if (typeof path !== 'string')
		// TODO: Should adding multiple locations simultaneously be implemented?
		throw new Error('Adding multiple locations simultaneously is not supported');

	return path;
};

export interface AddLocationDialog extends UseDialogProps {
	path: string;
	method?: RemoteErrorFormMessage;
}

export const AddLocationDialog = ({
	path,
	method = 'CREATE',
	...dialogProps
}: AddLocationDialog) => {
	const platform = usePlatform();
	const listLocations = useLibraryQuery(['locations.list']);
	const createLocation = useLibraryMutation('locations.create');
	const relinkLocation = useLibraryMutation('locations.relink');
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const addLocationToLibrary = useLibraryMutation('locations.addLibrary');

	// This is required because indexRules is undefined on first render
	const indexerRulesIds = useMemo(
		() => listIndexerRules.data?.filter((rule) => rule.default).map((rule) => rule.id) ?? [],
		[listIndexerRules.data]
	);

	const form = useZodForm({ schema, defaultValues: { path, method, indexerRulesIds } });

	useEffect(() => {
		// Update form values when default value changes and the user hasn't made any changes
		if (!form.formState.isDirty)
			form.reset({ path, method, indexerRulesIds }, { keepErrors: true });
	}, [form, path, method, indexerRulesIds]);

	const addLocation = useCallback(
		async ({ path, method, indexerRulesIds }: SchemaType, dryRun = false) => {
			switch (method) {
				case 'CREATE':
					await createLocation.mutateAsync({
						path,
						dry_run: dryRun,
						indexer_rules_ids: indexerRulesIds
					});
					break;
				case 'NEED_RELINK':
					if (!dryRun) await relinkLocation.mutateAsync(path);
					// TODO: Update relinked location with new indexer rules, don't have a way to get location id yet though
					// await updateLocation.mutateAsync({
					// 	id: locationId,
					// 	name: null,
					// 	hidden: null,
					// 	indexer_rules_ids,
					// 	sync_preview_media: null,
					// 	generate_preview_media: null
					// });
					break;
				case 'ADD_LIBRARY':
					await addLocationToLibrary.mutateAsync({
						path,
						dry_run: dryRun,
						indexer_rules_ids: indexerRulesIds
					});
					break;
				default:
					throw new Error('Unimplemented custom remote error handling');
			}
		},
		[createLocation, relinkLocation, addLocationToLibrary]
	);

	const handleAddError = useCallback(
		(error: unknown) => {
			const rspcErrorInfo = extractInfoRSPCError(error);
			if (!rspcErrorInfo || rspcErrorInfo.code === 500) return false;

			let { message } = rspcErrorInfo;
			if (rspcErrorInfo.code == 409 && isRemoteErrorFormMessage(message)) {
				/**
				 * TODO: On NEED_RELINK, we should query the backend for
				 * the current location indexer_rules_ids, then update the checkboxes
				 * accordingly. However we don't have the location id at this point.
				 * Maybe backend could return the location id in the error?
				 */
				if (form.getValues().method !== message) {
					form.setValue('method', message);
					message = REMOTE_ERROR_FORM_MESSAGE[message];
				} else {
					message = '';
				}
			}

			if (message && get(form.formState.errors, REMOTE_ERROR_FORM_FIELD)?.message !== message)
				form.setError(REMOTE_ERROR_FORM_FIELD, { type: 'remote', message: message });
			return true;
		},
		[form]
	);

	useCallbackToWatchForm(
		async (values, { name }) => {
			if (name === 'path') {
				// Remote errors should only be cleared when path changes,
				// as the previous error is used to notify the user of this change
				form.clearErrors(REMOTE_ERROR_FORM_FIELD);

				// Reset method when path changes
				if (form.getValues().method !== method) form.setValue('method', method);
			}

			if (values.path === '') return;

			try {
				await addLocation(values, true);
			} catch (error) {
				handleAddError(error);
			}
		},
		[form, method, addLocation, handleAddError]
	);

	const onSubmit: Parameters<typeof form.handleSubmit>[0] = async (values) => {
		try {
			await addLocation(values);
		} catch (error) {
			if (handleAddError(error)) {
				// Reset form to remove isSubmitting state
				form.reset({}, { keepValues: true, keepErrors: true, keepIsValid: true });
				// Throw error to prevent dialog from closing
				throw error;
			}

			showAlertDialog({
				title: 'Error',
				value: String(error) || 'Failed to add location'
			});

			return;
		}

		await listLocations.refetch();
	};

	return (
		<Dialog
			form={form}
			title="New Location"
			dialog={useDialog(dialogProps)}
			onSubmit={onSubmit}
			ctaLabel="Add"
			description={
				platform.platform === 'web'
					? 'As you are using the browser version of Spacedrive you will (for now) ' +
					  'need to specify an absolute URL of a directory local to the remote node.'
					: ''
			}
		>
			<ErrorMessage name={REMOTE_ERROR_FORM_FIELD} variant="large" className="mb-4 mt-2" />

			<Input
				size="md"
				label="Path:"
				onClick={() =>
					openDirectoryPickerDialog(platform)
						.then((path) => path && form.setValue('path', path))
						.catch((error) => showAlertDialog({ title: 'Error', value: String(error) }))
				}
				readOnly={platform.platform !== 'web'}
				className={clsx('mb-3', platform.platform === 'web' || 'cursor-pointer')}
				{...form.register('path')}
			/>

			<input type="hidden" {...form.register('method')} />

			<Controller
				name="indexerRulesIds"
				render={({ field }) => (
					<IndexerRuleEditor
						field={field}
						label="File indexing rules:"
						className="relative flex flex-col"
					/>
				)}
				control={form.control}
			/>
		</Dialog>
	);
};
