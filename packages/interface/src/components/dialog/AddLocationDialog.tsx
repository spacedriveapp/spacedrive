import { useLibraryMutation } from '@sd/client';
import { Input } from '@sd/ui';
import { Dialog } from '@sd/ui';
import { forms } from '@sd/ui';

const { useZodForm, z } = forms;

const schema = z.object({ path: z.string() });

interface Props {
	open: boolean;
	setOpen: (state: boolean) => void;
}

export default function AddLocationDialog({ open, setOpen }: Props) {
	const createLocation = useLibraryMutation('locations.create', {
		onSuccess: () => setOpen(false)
	});

	const form = useZodForm({
		schema,
		defaultValues: {
			// BEFORE MERGE: Remove default value
			path: '/Users/jamie/Projects/spacedrive/packages/test-files/files'
		}
	});

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(async ({ path }) => {
				await createLocation.mutateAsync({
					path,
					indexer_rules_ids: []
				});
			})}
			open={open}
			setOpen={setOpen}
			title="Add Location URL"
			description="As you are using the browser version of Spacedrive you will (for now) need to specify an absolute URL of a directory local to the remote node."
			loading={createLocation.isLoading}
			ctaLabel="Add"
			trigger={null}
		>
			<Input
				className="flex-grow w-full mt-3"
				placeholder="/Users/jamie/Movies"
				required
				{...form.register('path')}
			/>
		</Dialog>
	);
}
