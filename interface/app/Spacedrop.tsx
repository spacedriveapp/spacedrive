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

const { Input, useZodForm, z } = forms;

export function SpacedropUI() {
	useEffect(() =>
		subscribeSpacedropState(() => {
			dialogManager.create((dp) => <SpacedropDialog {...dp} />);
		})
	);

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			if (data.type === 'SpacedropRequest') {
				dialogManager.create((dp) => (
					<SpacedropRequestDialog
						dropId={data.id}
						name={data.name}
						peerId={data.peer_id}
						{...dp}
					/>
				));
			}
		}
	});

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
	const onSubmit = form.handleSubmit((data) =>
		doSpacedrop.mutate({
			file_path: getSpacedropState().droppedFiles,
			peer_id: data.target_peer
		})
	);

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

function SpacedropRequestDialog(
	props: { dropId: string; name: string; peerId: string } & UseDialogProps
) {
	const dialog = useDialog(props);
	const form = useZodForm({
		// We aren't using this but it's required for the Dialog :(
		schema: z.object({
			file_path: z.string()
		})
	});

	const acceptSpacedrop = useBridgeMutation('p2p.acceptSpacedrop');
	const onSubmit = form.handleSubmit((data) =>
		acceptSpacedrop.mutate([props.dropId, data.file_path])
	);

	// TODO: Automatically close this after 60 seconds cause the Spacedrop would have expired

	return (
		<Dialog
			form={form}
			dialog={dialog}
			title="Received Spacedrop"
			loading={acceptSpacedrop.isLoading}
			ctaLabel="Send"
			closeLabel="Cancel"
			onSubmit={onSubmit}
			onCancelled={() => acceptSpacedrop.mutate([props.dropId, null])}
		>
			<div className="space-y-2 py-2">
				<p>File Name: {props.name}</p>
				<p>Peer Id: {props.peerId}</p>
				<Input
					size="sm"
					placeholder="/Users/oscar/Desktop/demo.txt"
					className="w-full"
					{...form.register('file_path')}
				/>
			</div>
		</Dialog>
	);
}
