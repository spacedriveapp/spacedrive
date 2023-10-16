import { useEffect, useMemo, useRef, useState } from 'react';
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

const placeholder = '/Users/oscar/Desktop/demo.txt';
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
								File '{data.files[0]}' from '{data.peer_name}'
							</p>
							{/* TODO: This will be removed in the future for now it's just a hack */}
							{platform.saveFilePickerDialog ? null : (
								<Input
									ref={filePathInput}
									name="file_path"
									size="sm"
									placeholder={placeholder}
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
							let destinationFilePath = filePathInput.current?.value ?? placeholder;

							if (data.files.length != 1) {
								if (platform.openDirectoryPickerDialog) {
									const result = await platform.openDirectoryPickerDialog({
										title: 'Save Spacedrop',
										multiple: false
									});
									if (!result) {
										return;
									}
									destinationFilePath = result;
								}
							} else {
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
							}

							if (destinationFilePath === '') return;
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

	useEffect(() => {
		let open = false;

		return subscribeSpacedropState(() => {
			if (open) return;
			open = true;
			dialogManager.create((dp) => <SpacedropDialog {...dp} />).then(() => (open = false));
		});
	});

	return null;
}

function SpacedropDialog(props: UseDialogProps) {
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
					file_path: getSpacedropState().droppedFiles,
					peer_id: data.targetPeer! // `submitDisabled` ensures this
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
