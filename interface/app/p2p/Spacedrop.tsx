import { useEffect, useRef, useState } from 'react';
import {
	useBridgeMutation,
	useDiscoveredPeers,
	useP2PEvents,
	useSpacedropProgress,
	useZodForm
} from '@sd/client';
import {
	Dialog,
	dialogManager,
	Input,
	ProgressBar,
	SelectField,
	SelectOption,
	toast,
	ToastId,
	useDialog,
	UseDialogProps,
	z
} from '@sd/ui';
import { usePlatform } from '~/util/Platform';

import { getSpacedropState, subscribeSpacedropState } from '../../hooks/useSpacedropState';

function SpacedropProgress({ toastId, dropId }: { toastId: ToastId; dropId: string }) {
	const progress = useSpacedropProgress(dropId);

	useEffect(() => {
		if (progress === 100) {
			setTimeout(() => toast.dismiss(toastId), 750);
		}
	}, [progress, toastId]);

	return (
		<div className="pt-1">
			<ProgressBar percent={progress ?? 0} />
		</div>
	);
}

export function SpacedropUI() {
	const platform = usePlatform();
	const cancelSpacedrop = useBridgeMutation(['p2p.cancelSpacedrop']);
	const acceptSpacedrop = useBridgeMutation('p2p.acceptSpacedrop');
	const filePathInput = useRef<HTMLInputElement>(null);

	useP2PEvents((data) => {
		if (data.type === 'SpacedropRequest') {
			toast.info(
				{
					title: 'Incoming Spacedrop',
					// TODO: Make this pretty
					body: (
						<>
							<p>
								File '{data.file_name}' from '{data.peer_name}'
							</p>
							{/* TODO: This will be removed in the future for now it's just a hack */}
							{platform.saveFilePickerDialog ? null : (
								<Input
									ref={filePathInput}
									name="file_path"
									size="sm"
									placeholder="/Users/oscar/Desktop/demo.txt"
									className="w-full"
								/>
							)}
							{/* TODO: Button to expand the toast and show the entire PeerID for manual verification? */}
						</>
					)
				},
				{
					duration: 30 * 1000,
					onClose: ({ event }) => {
						event !== 'on-action' && acceptSpacedrop.mutate([data.id, null]);
					},
					action: {
						label: 'Accept',
						async onClick() {
							let destinationFilePath = filePathInput.current?.value ?? '';
							if (platform.saveFilePickerDialog) {
								const result = await platform.saveFilePickerDialog({
									title: 'Save Spacedrop',
									defaultPath: data.file_name
								});
								if (!result) {
									return;
								}
								destinationFilePath = result;
							}

							await acceptSpacedrop.mutateAsync([data.id, destinationFilePath]);
						}
					},
					cancel: 'Reject'
				}
			);
		} else if (data.type === 'SpacedropProgress') {
			toast.info(
				(id) => ({
					title: 'Spacedrop',
					body: <SpacedropProgress toastId={id} dropId={data.id} />
				}),
				{
					id: data.id,
					duration: Infinity,
					cancel: {
						label: 'Cancel',
						onClick() {
							cancelSpacedrop.mutate(data.id);
						}
					}
				}
			);
		} else if (data.type === 'SpacedropRejected') {
			// TODO: Add more information to this like peer name, etc in future
			toast.warning('Spacedrop Rejected');
		}
	});

	useEffect(() =>
		subscribeSpacedropState(() => {
			dialogManager.create((dp) => <SpacedropDialog {...dp} />);
		})
	);

	return null;
}

function SpacedropDialog(props: UseDialogProps) {
	const discoveredPeers = useDiscoveredPeers();
	const form = useZodForm({
		// We aren't using this but it's required for the Dialog :(
		schema: z.object({
			targetPeer: z.string()
		})
	});

	const doSpacedrop = useBridgeMutation('p2p.spacedrop');

	useEffect(() => {
		if (!form.getValues('targetPeer')) {
			const [peerId] = [...discoveredPeers.entries()][0] ?? [];
			if (peerId) {
				form.setValue('targetPeer', peerId);
			}
		}
	}, [form, discoveredPeers]);

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
					peer_id: data.targetPeer
				})
			)}
		>
			<div className="space-y-2 py-2">
				<SelectField name="targetPeer">
					{[...discoveredPeers.entries()].map(([peerId, metadata], index) => (
						<SelectOption key={peerId} value={peerId}>
							{metadata.name}
						</SelectOption>
					))}
				</SelectField>
			</div>
		</Dialog>
	);
}
