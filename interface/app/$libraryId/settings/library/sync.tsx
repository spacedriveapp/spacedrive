import { inferSubscriptionResult } from '@oscartbeaumont-sd/rspc-client';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import {
	Procedures,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription,
	useZodForm
} from '@sd/client';
import { Button, Dialog, dialogManager, useDialog, UseDialogProps, z } from '@sd/ui';
import { useLocale } from '~/hooks';

import { Heading } from '../Layout';
import Setting from '../Setting';

const ACTORS = {
	Ingest: 'Sync Ingest',
	CloudSend: 'Cloud Sync Sender',
	CloudReceive: 'Cloud Sync Receiver',
	CloudIngest: 'Cloud Sync Ingest'
};

export const Component = () => {
	const { t } = useLocale();

	const syncEnabled = useLibraryQuery(['sync.enabled']);

	const backfillSync = useLibraryMutation(['sync.backfill'], {
		onSuccess: async () => {
			await syncEnabled.refetch();
		}
	});

	const [data, setData] = useState<inferSubscriptionResult<Procedures, 'library.actors'>>({});

	useLibrarySubscription(['library.actors'], { onData: setData });

	return (
		<>
			<Heading title={t('sync')} description={t('sync_description')} />
			{syncEnabled.data === false ? (
				<Setting
					mini
					title="Enable Sync"
					description="Generate sync operations for all the existing data in this library, and configure Spacedrive to generate sync operations when things happen in future."
				>
					<div>
						<Button
							className="text-nowrap"
							variant="accent"
							onClick={() => {
								dialogManager.create((dialogProps) => <SyncBackfillDialog {...dialogProps} />);
							}}
							disabled={backfillSync.isLoading}
						>
							Enable sync
						</Button>
					</div>
				</Setting>
			) : (
				<>
					<Setting
						mini
						title={
							<>
								Ingester
								<OnlineIndicator online={data[ACTORS.Ingest] ?? false} />
							</>
						}
						description="This process takes sync operations from P2P connections and Spacedrive Cloud and applies them to the library."
					>
						<div>
							{data[ACTORS.Ingest] ? (
								<StopButton name={ACTORS.Ingest} />
							) : (
								<StartButton name={ACTORS.Ingest} />
							)}
						</div>
					</Setting>
					<CloudSync data={data} />
				</>
			)}
		</>
	);
};

function SyncBackfillDialog(props: UseDialogProps) {
	const form = useZodForm({ schema: z.object({}) });
	const dialog = useDialog(props);

	const enableSync = useLibraryMutation(['sync.backfill'], {});

	// dialog is in charge of enabling sync
	useEffect(() => {
		form.handleSubmit(
			async () => {
				await enableSync.mutateAsync(null).then(() => (dialog.state.open = false));
			},
			() => {}
		)();
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	return (
		<Dialog
			title="Backfilling Sync Operations"
			description="Library is paused until backfill completes"
			form={form}
			dialog={dialog}
			hideButtons
			ignoreClickOutside
		/>
	);
}

function CloudSync({ data }: { data: inferSubscriptionResult<Procedures, 'library.actors'> }) {
	return (
		<>
			<div>
				<h1 className="mb-0.5 text-lg font-bold text-ink">Cloud Sync</h1>
				<p className="text-sm text-ink-faint">
					Manage the processes that sync your library with Spacedrive Cloud
				</p>
			</div>
			<Setting
				mini
				title={
					<>
						Sender <OnlineIndicator online={data[ACTORS.CloudSend] ?? false} />
					</>
				}
				description="This process sends sync operations to Spacedrive Cloud."
			>
				<div>
					{data[ACTORS.CloudSend] ? (
						<StopButton name={ACTORS.CloudSend} />
					) : (
						<StartButton name={ACTORS.CloudSend} />
					)}
				</div>
			</Setting>
			<Setting
				mini
				title={
					<>
						Receiver
						<OnlineIndicator online={data[ACTORS.CloudReceive] ?? false} />
					</>
				}
				description="This process receives and stores operations from Spacedrive Cloud."
			>
				<div>
					{data[ACTORS.CloudReceive] ? (
						<StopButton name={ACTORS.CloudReceive} />
					) : (
						<StartButton name={ACTORS.CloudReceive} />
					)}
				</div>
			</Setting>
			<Setting
				mini
				title={
					<>
						Ingester
						<OnlineIndicator online={data[ACTORS.CloudIngest] ?? false} />
					</>
				}
				description="This process takes received cloud operations and sends them to the main sync ingester."
			>
				<div>
					{data[ACTORS.CloudIngest] ? (
						<StopButton name={ACTORS.CloudIngest} />
					) : (
						<StartButton name={ACTORS.CloudIngest} />
					)}
				</div>
			</Setting>
		</>
	);
}

function StartButton({ name }: { name: string }) {
	const startActor = useLibraryMutation(['library.startActor']);

	return (
		<Button
			variant="accent"
			disabled={startActor.isLoading}
			onClick={() => startActor.mutate(name)}
		>
			{startActor.isLoading ? 'Starting...' : 'Start'}
		</Button>
	);
}

function StopButton({ name }: { name: string }) {
	const stopActor = useLibraryMutation(['library.stopActor']);

	return (
		<Button variant="accent" disabled={stopActor.isLoading} onClick={() => stopActor.mutate(name)}>
			{stopActor.isLoading ? 'Stopping...' : 'Stop'}
		</Button>
	);
}

function OnlineIndicator({ online }: { online: boolean }) {
	return (
		<div
			className={clsx(
				'ml-1.5 inline-block size-2.5 rounded-full',
				online ? 'bg-green-500' : 'bg-red-500'
			)}
		/>
	);
}
