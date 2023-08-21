import { useState } from 'react';
import { P, match } from 'ts-pattern';
import {
	OperatingSystem,
	useBridgeMutation,
	useCachedLibraries,
	usePairingStatus
} from '@sd/client';
import {
	Button,
	Dialog,
	Loader,
	Select,
	SelectOption,
	UseDialogProps,
	dialogManager,
	useDialog,
	useZodForm,
	z
} from '@sd/ui';

type Node = {
	name: string;
	os: OperatingSystem | null;
};

export function startPairing(pairing_id: number, node: Node) {
	dialogManager.create((dp) => <OriginatorDialog pairingId={pairing_id} node={node} {...dp} />);
}

function OriginatorDialog({
	pairingId,
	node,
	...props
}: { pairingId: number; node: Node } & UseDialogProps) {
	const pairingStatus = usePairingStatus(pairingId);

	// TODO: If dialog closes before finished, cancel pairing

	return (
		<Dialog
			form={useZodForm({ schema: z.object({}) })}
			dialog={useDialog(props)}
			title={`Pairing with ${node.name}`}
			loading={true}
			submitDisabled={pairingStatus?.type !== 'PairingComplete'}
			ctaLabel="Done"
			// closeLabel="Cancel"
			onSubmit={async () => {
				// TODO: Change into the new library
			}}
			// onCancelled={() => acceptSpacedrop.mutate([props.dropId, null])}
		>
			<div className="space-y-2 py-2">
				{match(pairingStatus)
					.with({ type: 'EstablishingConnection' }, () => (
						<PairingLoading msg="Establishing connection..." />
					))
					.with({ type: 'PairingRequested' }, () => (
						<PairingLoading msg="Requesting to pair..." />
					))
					.with({ type: 'LibraryAlreadyExists' }, () => (
						<PairingLoading msg={`Pairing failed due to library already existing!`} />
					))
					.with({ type: 'PairingDecisionRequest' }, () => (
						<PairingResponder pairingId={pairingId} />
					))
					.with({ type: 'PairingInProgress', data: P.select() }, (data) => (
						<PairingLoading msg={`Pairing into library ${data.library_name}`} />
					))
					.with({ type: 'InitialSyncProgress', data: P.select() }, (data) => (
						<PairingLoading msg={`Syncing library data ${data}/100`} />
					))
					.with({ type: 'PairingComplete' }, () => <CompletePairing />)
					.with({ type: 'PairingRejected' }, () => <PairingRejected />)
					.with(undefined, () => <></>)
					.exhaustive()}
			</div>
		</Dialog>
	);
}

function PairingResponder({ pairingId }: { pairingId: number }) {
	const libraries = useCachedLibraries();
	const [selectedLibrary, setSelectedLibrary] = useState<string | undefined>(
		libraries.data?.[0]?.uuid
	);
	const pairingResponse = useBridgeMutation('p2p.pairingResponse');

	return (
		<>
			{selectedLibrary ? (
				<Select onChange={(e) => setSelectedLibrary(e)} value={selectedLibrary}>
					{libraries.data?.map((lib, index) => (
						<SelectOption default={index === 0} key={lib.uuid} value={lib.uuid}>
							{lib.config.name}
						</SelectOption>
					))}
				</Select>
			) : (
				<p>No libraries. Uh oh!</p>
			)}
			<div className="align-center flex h-full w-full items-center justify-center space-x-2">
				<Button
					variant="accent"
					onClick={() => {
						if (selectedLibrary)
							pairingResponse.mutate([
								pairingId,
								{ decision: 'accept', libraryId: selectedLibrary }
							]);
					}}
				>
					Accept
				</Button>
				<Button onClick={() => pairingResponse.mutate([pairingId, { decision: 'reject' }])}>
					Reject
				</Button>
			</div>
		</>
	);
}

function PairingLoading({ msg }: { msg?: string }) {
	return (
		<div className="align-center flex h-full w-full flex-col items-center justify-center">
			<Loader />
			{msg && <p>{msg}</p>}
		</div>
	);
}

function CompletePairing() {
	return (
		<div className="flex h-full w-full justify-center">
			<p>Pairing Complete!</p>
		</div>
	);
}

function PairingRejected() {
	return (
		<div className="flex h-full w-full justify-center">
			<p>Pairing Rejected By Remote!</p>
		</div>
	);
}
