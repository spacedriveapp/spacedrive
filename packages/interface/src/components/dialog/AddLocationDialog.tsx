import { useLibraryMutation } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Input, useZodForm, z } from '@sd/ui/src/forms';

const schema = z.object({ path: z.string() });

type Props = UseDialogProps;

export default function AddLocationDialog(props: Props) {
	const dialog = useDialog(props);
	const createLocation = useLibraryMutation('locations.create');

	const form = useZodForm({
		schema,
		defaultValues: {
			// BEFORE MERGE: Remove default value
			path: '/Users/jamie/Projects/spacedrive/packages/test-files/files'
		}
	});

	return (
		<Dialog
			{...{ dialog, form }}
			onSubmit={form.handleSubmit(({ path }) =>
				createLocation.mutateAsync({
					path,
					indexer_rules_ids: []
				})
			)}
			title="Add Location URL"
			description="As you are using the browser version of Spacedrive you will (for now) need to specify an absolute URL of a directory local to the remote node."
			ctaLabel="Add"
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
