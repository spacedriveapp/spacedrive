import { useEffect, useState } from 'react';
import {
	useBridgeMutation,
	useDiscoveredPeers,
	useP2PEvents,
	useSpacedropProgress,
	useZodForm
} from '@sd/client';
import {
	Button,
	Dialog,
	InputField,
	ProgressBar,
	Select,
	SelectOption,
	UseDialogProps,
	dialogManager,
	toast,
	useDialog,
	z
} from '@sd/ui';
import { usePlatform } from '~/util/Platform';
import { getSpacedropState, subscribeSpacedropState } from '../../hooks/useSpacedropState';

function SpacedropProgress({ toastId, dropId }: { toastId: string | number; dropId: string }) {
	const progress = useSpacedropProgress(dropId);

	useEffect(() => {
		if (progress === 100) {
			setTimeout(() => toast.dismiss(toastId), 750);
		}
	});

	return (
		<div className="pt-1">
			<ProgressBar percent={progress ?? 0} />
		</div>
	);
}

export function SpacedropUI() {
	const cancelSpacedrop = useBridgeMutation(['p2p.cancelSpacedrop']);

	useEffect(() =>
		subscribeSpacedropState(() => {
			dialogManager.create((dp) => <SpacedropDialog {...dp} />);
		})
	);

	const [[spacedropToasts], _] = useState([new Map<string, null>()]);
	useP2PEvents((data) => {
		if (data.type === 'SpacedropProgress') {
			if (!spacedropToasts.has(data.id)) {
				toast.info(
					{
						title: 'Spacedrop',
						description: (id) => <SpacedropProgress toastId={id} dropId={data.id} />
					},
					{
						duration: Infinity,
						cancel: {
							label: 'Cancel',
							onClick() {
								cancelSpacedrop.mutate(data.id);
							}
						}
					}
				);
				spacedropToasts.set(data.id, null);
			}
		}
	});

	useP2PEvents((data) => {
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
	});

	return null;
}

function SpacedropDialog(props: UseDialogProps) {
	const discoveredPeers = useDiscoveredPeers();
	const form = useZodForm({
		// We aren't using this but it's required for the Dialog :(
		schema: z.object({
			target_peer: z.string()
		})
	});

	const doSpacedrop = useBridgeMutation('p2p.spacedrop');

	return (
		<Dialog
			form={form}
			dialog={useDialog(props)}
			title="Spacedrop a File"
			loading={doSpacedrop.isLoading}
			ctaLabel="Send"
			closeLabel="Cancel"
			onSubmit={form.handleSubmit((data) =>
				doSpacedrop.mutateAsync({
					file_path: getSpacedropState().droppedFiles,
					peer_id: data.target_peer
				})
			)}
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
	const platform = usePlatform();
	const form = useZodForm({
		// We aren't using this but it's required for the Dialog :(
		schema: z.object({
			file_path: z.string()
		})
	});

	const acceptSpacedrop = useBridgeMutation('p2p.acceptSpacedrop');

	// TODO: Automatically close this after 60 seconds cause the Spacedrop would have expired

	return (
		<Dialog
			form={form}
			dialog={useDialog(props)}
			title="Received Spacedrop"
			loading={acceptSpacedrop.isLoading}
			ctaLabel="Accept"
			closeLabel="Decline"
			onSubmit={async (data) => {
				if (platform.saveFilePickerDialog) {
					const result = await platform.saveFilePickerDialog({
						title: 'Save Spacedrop',
						defaultPath: props.name
					});

					if (!result) {
						return;
					}

					form.setValue('file_path', result);
				}

				return await form.handleSubmit((data) =>
					acceptSpacedrop.mutateAsync([props.dropId, data.file_path])
				)(data);
			}}
			onCancelled={() => acceptSpacedrop.mutate([props.dropId, null])}
		>
			<div className="space-y-2 py-2">
				<p>File Name: {props.name}</p>
				<p>Peer Id: {props.peerId}</p>
				{platform.saveFilePickerDialog ? null : (
					<InputField
						size="sm"
						placeholder="/Users/oscar/Desktop/demo.txt"
						className="w-full"
						{...form.register('file_path')}
					/>
				)}
			</div>
		</Dialog>
	);
}
