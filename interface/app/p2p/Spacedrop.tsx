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
	Input,
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
	const platform = usePlatform();
	const cancelSpacedrop = useBridgeMutation(['p2p.cancelSpacedrop']);
	const acceptSpacedrop = useBridgeMutation('p2p.acceptSpacedrop');
	const [[spacedropToasts], _] = useState([new Map<string, null>()]);
	const filePathInput = useRef<HTMLInputElement>(null);

	useP2PEvents((data) => {
		if (data.type === 'SpacedropRequest') {
			toast.info(
				{
					title: 'Incoming Spacedrop',
					// TODO: Make this pretty
					description: () => {
						return (
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
						);
					}
				},
				{
					duration: 30 * 1000,
					onDismiss: () => {
						acceptSpacedrop.mutate([data.id, null]);
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
					cancel: {
						label: 'Reject',
						onClick() {
							acceptSpacedrop.mutate([data.id, null]);
						}
					}
				}
			);
		} else if (data.type === 'SpacedropProgress') {
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
