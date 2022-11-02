import { LocationCreateArgs, useBridgeMutation, useLibraryMutation } from '@sd/client';
import { Input } from '@sd/ui';
import { Dialog } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { PropsWithChildren, useState } from 'react';

export default function AddLocationDialog({
	children,
	onSubmit,
	open,
	setOpen
}: PropsWithChildren<{ onSubmit?: () => void; open: boolean; setOpen: (state: boolean) => void }>) {
	// BEFORE MERGE: Remove default value
	const [locationUrl, setLocationUrl] = useState(
		'/Users/jamie/Projects/spacedrive/packages/test-files/files'
	);

	const createLocation = useLibraryMutation('locations.create', {
		onSuccess: () => setOpen(false)
	});

	return (
		<Dialog
			open={open}
			setOpen={setOpen}
			title="Add Location URL"
			description="As you are using the browser version of Spacedrive you will (for now) need to specify an absolute URL of a directory local to the remote node."
			ctaAction={() =>
				createLocation.mutate({
					path: locationUrl,
					indexer_rules_ids: []
				} as LocationCreateArgs)
			}
			loading={createLocation.isLoading}
			submitDisabled={!locationUrl}
			ctaLabel="Add"
			trigger={null}
		>
			<Input
				className="flex-grow w-full mt-3"
				value={locationUrl}
				placeholder="/Users/jamie/Movies"
				onChange={(e) => setLocationUrl(e.target.value)}
				required
			/>
		</Dialog>
	);
}
