import { useEffect, useState } from 'react';
import { PeerMetadata, useBridgeMutation, useBridgeSubscription } from '@sd/client';
import {
	Dialog,
	Select,
	SelectOption,
	UseDialogProps,
	dialogManager,
	forms,
	useDialog
} from '@sd/ui';
import { getSpacedropState, subscribeSpacedropState } from '../hooks/useSpacedropState';

const { useZodForm, z } = forms;

export function SpacedropUI() {
	useEffect(() =>
		subscribeSpacedropState(() => {
			dialogManager.create((dp) => <SpacedropDialog {...dp} />);
		})
	);

	return null;
}

function SpacedropDialog(props: UseDialogProps) {
	const [[discoveredPeers], setDiscoveredPeer] = useState([new Map<string, PeerMetadata>()]);
	const dialog = useDialog(props);
	const form = useZodForm({
		// We aren't using this but it's required for the Dialog :(
		schema: z.object({
			target_peer: z.string()
		})
	});

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			if (data.type === 'DiscoveredPeer') {
				setDiscoveredPeer([discoveredPeers.set(data.peer_id, data.metadata)]);
			}
		}
	});

	const doSpacedrop = useBridgeMutation('p2p.spacedrop');
	const onSubmit = form.handleSubmit((data) => {
		console.log('HERE');
		doSpacedrop.mutate({
			file_path: getSpacedropState().droppedFiles,
			peer_id: data.target_peer
		});
	});

	return (
		<Dialog
			form={form}
			dialog={dialog}
			title="Spacedrop a File"
			loading={doSpacedrop.isLoading}
			ctaLabel="Send"
			closeLabel="Cancel"
			onSubmit={onSubmit}
		>
			<div className="space-y-2 py-2">
				<Select
					onChange={(e) => form.setValue('target_peer', e)}
					value={form.watch('target_peer')}
				>
					{[...discoveredPeers.entries()].map(([peerId, metadata], index) => (
						<SelectOption default={index === 0} key={peerId} value={peerId}>
							{metadata.name}
						</SelectOption>
					))}
				</Select>
			</div>
		</Dialog>
	);
}
