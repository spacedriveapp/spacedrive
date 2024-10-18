import {
	MaybeUndefined,
	useBridgeMutation,
	useLibraryContext,
	useLibraryMutation,
	useZodForm
} from '@sd/client';
import { Button, dialogManager, Form, InputField, Switch, Tooltip, z } from '@sd/ui';
import { useDebouncedFormWatch, useLocale } from '~/hooks';

import { Heading } from '../Layout';
import DeleteLibraryDialog from '../node/libraries/DeleteDialog';
import Setting from '../Setting';

const schema = z.object({
	id: z.string(),
	name: z.string().min(1),
	description: z.string().nullable()
});

// TODO: With some extra upstream Specta work this should be able to be removed
function toMaybeUndefined<T>(v: T | null | undefined): MaybeUndefined<T> {
	return v as any;
}

export const Component = () => {
	const { library } = useLibraryContext();
	const editLibrary = useBridgeMutation('library.edit');
	const vacuumLibrary = useLibraryMutation('library.vacuumDb');

	const { t } = useLocale();

	const form = useZodForm({
		schema,
		defaultValues: {
			id: library!.uuid,
			...library?.config
		},
		mode: 'onChange'
	});
	const { isValid } = form.formState;

	useDebouncedFormWatch(form, (value) => {
		if (!isValid) return;
		editLibrary.mutate({
			id: library.uuid,
			name: value.name ?? null,
			description: toMaybeUndefined(value.description)
		});
	});

	return (
		<Form form={form}>
			<div className="flex w-full max-w-4xl flex-col space-y-6 pb-5">
				<Heading
					title={t('library_settings')}
					description={t('library_settings_description')}
				/>

				<input type="hidden" {...form.register('id')} />

				<div className="flex flex-row space-x-5 pb-3">
					<InputField
						size="md"
						label={t('name')}
						formFieldClassName="flex-1"
						defaultValue="My Default Library"
						{...form.register('name', { required: true })}
					/>
					<InputField
						label={t('description')}
						size="md"
						formFieldClassName="flex-1"
						{...form.register('description')}
					/>
				</div>

				<Setting
					mini
					title={t('encrypt_library')}
					description={t('encrypt_library_description')}
				>
					<div className="ml-3 flex items-center">
						<Tooltip label={t('encrypt_library_coming_soon')}>
							<Switch disabled size="md" checked={false} />
						</Tooltip>
					</div>
				</Setting>

				<Setting
					mini
					title={t('export_library')}
					description={t('export_library_description')}
				>
					<div className="mt-2">
						<Tooltip label={t('export_library_coming_soon')}>
							<Button disabled size="sm" variant="gray" className="whitespace-nowrap">
								{t('export')}
							</Button>
						</Tooltip>
					</div>
				</Setting>

				<Setting
					mini
					title={t('vacuum_library')}
					description={t('vacuum_library_description')}
				>
					<div className="mt-2">
						<Button
							onClick={() => vacuumLibrary.mutate(null)}
							disabled={vacuumLibrary.isPending}
							size="sm"
							variant="gray"
							className="whitespace-nowrap"
						>
							{t('vacuum')}
						</Button>
					</div>
				</Setting>

				<Setting
					mini
					title={t('delete_library')}
					description={t('delete_library_description')}
				>
					<div className="mt-2">
						<Button
							size="sm"
							variant="colored"
							className="whitespace-nowrap border-red-500 bg-red-500"
							onClick={() => {
								dialogManager.create((dp) => (
									<DeleteLibraryDialog {...dp} libraryUuid={library.uuid} />
								));
							}}
						>
							{t('delete')}
						</Button>
					</div>
				</Setting>
				<div className="block h-20" />
			</div>
		</Form>
	);
};
