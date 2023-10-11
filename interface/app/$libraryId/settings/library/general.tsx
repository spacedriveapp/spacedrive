import { MaybeUndefined, useBridgeMutation, useLibraryContext, useZodForm } from '@sd/client';
import { Button, dialogManager, Form, InputField, Switch, Tooltip, z } from '@sd/ui';
import { useDebouncedFormWatch } from '~/hooks';

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
					title="Library Settings"
					description="General settings related to the currently active library."
				/>

				<input type="hidden" {...form.register('id')} />

				<div className="flex flex-row space-x-5 pb-3">
					<InputField
						size="md"
						label="Name"
						formFieldClassName="flex-1"
						defaultValue="My Default Library"
						{...form.register('name', { required: true })}
					/>
					<InputField
						label="Description"
						size="md"
						formFieldClassName="flex-1"
						{...form.register('description')}
					/>
				</div>

				<Setting
					mini
					title="Encrypt Library"
					description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves."
				>
					<div className="ml-3 flex items-center">
						<Tooltip label="Library encryption coming soon">
							<Switch disabled size="md" checked={false} />
						</Tooltip>
					</div>
				</Setting>

				<Setting mini title="Export Library" description="Export this library to a file.">
					<div className="mt-2">
						<Tooltip label="Export Library coming soon">
							<Button disabled size="sm" variant="gray">
								Export
							</Button>
						</Tooltip>
					</div>
				</Setting>

				<Setting
					mini
					title="Delete Library"
					description="This is permanent, your files will not be deleted, only the Spacedrive library."
				>
					<div className="mt-2">
						<Button
							size="sm"
							variant="colored"
							className="border-red-500 bg-red-500"
							onClick={() => {
								dialogManager.create((dp) => (
									<DeleteLibraryDialog {...dp} libraryUuid={library.uuid} />
								));
							}}
						>
							Delete
						</Button>
					</div>
				</Setting>
				<div className="block h-20" />
			</div>
		</Form>
	);
};
