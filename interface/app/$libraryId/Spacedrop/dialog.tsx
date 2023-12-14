import { useEffect, useMemo } from 'react';
import { useBridgeMutation, useDiscoveredPeers, useZodForm } from '@sd/client';
import { Dialog, SelectField, SelectOption, useDialog, UseDialogProps, z } from '@sd/ui';

export function SpacedropDialog(props: UseDialogProps & { path: string[] }) {
	const discoveredPeers = useDiscoveredPeers();
	const discoveredPeersArray = useMemo(() => [...discoveredPeers.entries()], [discoveredPeers]);
	const form = useZodForm({
		mode: 'onChange',
		// We aren't using this but it's required for the Dialog :(
		schema: z.object({
			// This field is actually required but the Zod validator is not working with select's so this is good enough for now.
			targetPeer: z.string().optional()
		})
	});
	const value = form.watch('targetPeer');

	useEffect(() => {
		// If peer goes offline deselect it
		if (
			value !== undefined &&
			discoveredPeersArray.find(([peerId]) => peerId === value) === undefined
		)
			form.setValue('targetPeer', undefined);

		const defaultValue = discoveredPeersArray[0]?.[0];
		// If no peer is selected, select the first one
		if (value === undefined && defaultValue) form.setValue('targetPeer', defaultValue);
	}, [form, value, discoveredPeersArray]);

	const doSpacedrop = useBridgeMutation('p2p.spacedrop');

	return (
		<Dialog
			// This `key` is a hack to workaround https://linear.app/spacedriveapp/issue/ENG-1208/improve-dialogs
			key={props.id}
			form={form}
			dialog={useDialog(props)}
			title="Spacedrop a File"
			loading={doSpacedrop.isLoading}
			ctaLabel="Send"
			closeLabel="Cancel"
			onSubmit={form.handleSubmit((data) =>
				doSpacedrop.mutateAsync({
					file_path: props.path,
					identity: data.targetPeer! // `submitDisabled` ensures this
				})
			)}
			submitDisabled={value === undefined}
		>
			<div className="space-y-2 py-2">
				<SelectField name="targetPeer">
					{discoveredPeersArray.map(([peerId, metadata], index) => (
						<SelectOption key={peerId} value={peerId} default={index === 0}>
							{metadata.name}
						</SelectOption>
					))}
				</SelectField>
			</div>
		</Dialog>
	);
}
