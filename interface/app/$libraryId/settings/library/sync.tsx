import { inferSubscriptionResult } from '@oscartbeaumont-sd/rspc-client';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import {
	Procedures,
	useFeatureFlag,
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

	const cloudSync = useFeatureFlag('cloudSync');

	return (
		<>
			<Heading title={t('sync')} description={t('sync_description')} />
			{syncEnabled.data === false ? (
				<Setting mini title={t('enable_sync')} description={t('enable_sync_description')}>
					<div>
						<Button
							className="text-nowrap"
							variant="accent"
							onClick={() => {
								dialogManager.create((dialogProps) => (
									<SyncBackfillDialog
										onEnabled={() => syncEnabled.refetch()}
										{...dialogProps}
									/>
								));
							}}
							disabled={backfillSync.isLoading}
						>
							{t('enable_sync')}
						</Button>
					</div>
				</Setting>
			) : (
				<>
					<Setting
						mini
						title={
							<>
								{t('ingester')}
								<OnlineIndicator online={data[ACTORS.Ingest] ?? false} />
							</>
						}
						description={t('injester_description')}
					>
						<div>
							{data[ACTORS.Ingest] ? (
								<StopButton name={ACTORS.Ingest} />
							) : (
								<StartButton name={ACTORS.Ingest} />
							)}
						</div>
					</Setting>

					{cloudSync && <CloudSync data={data} />}
				</>
			)}
		</>
	);
};

function SyncBackfillDialog(props: UseDialogProps & { onEnabled: () => void }) {
	const form = useZodForm({ schema: z.object({}) });
	const dialog = useDialog(props);
	const { t } = useLocale();

	const enableSync = useLibraryMutation(['sync.backfill'], {});

	// dialog is in charge of enabling sync
	useEffect(() => {
		form.handleSubmit(
			async () => {
				await enableSync.mutateAsync(null).then(() => (dialog.state.open = false));
				await props.onEnabled();
			},
			() => {}
		)();
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	return (
		<Dialog
			title={t('backfill_sync')}
			description={t('backfill_sync_description')}
			form={form}
			dialog={dialog}
			hideButtons
			ignoreClickOutside
		/>
	);
}

function CloudSync({ data }: { data: inferSubscriptionResult<Procedures, 'library.actors'> }) {
	const { t } = useLocale();
	return (
		<>
			<div>
				<h1 className="mb-0.5 text-lg font-bold text-ink">{t('cloud_sync')}</h1>
				<p className="text-sm text-ink-faint">{t('cloud_sync_description')}</p>
			</div>
			<Setting
				mini
				title={
					<>
						{t('sender')} <OnlineIndicator online={data[ACTORS.CloudSend] ?? false} />
					</>
				}
				description={t('sender_description')}
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
						{t('receiver')}
						<OnlineIndicator online={data[ACTORS.CloudReceive] ?? false} />
					</>
				}
				description={t('receiver_description')}
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
						{t('ingester')}
						<OnlineIndicator online={data[ACTORS.CloudIngest] ?? false} />
					</>
				}
				description={t('ingester_description')}
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
	const { t } = useLocale();

	return (
		<Button
			variant="accent"
			disabled={startActor.isLoading}
			onClick={() => startActor.mutate(name)}
		>
			{startActor.isLoading ? t('starting') : t('start')}
		</Button>
	);
}

function StopButton({ name }: { name: string }) {
	const stopActor = useLibraryMutation(['library.stopActor']);
	const { t } = useLocale();

	return (
		<Button
			variant="accent"
			disabled={stopActor.isLoading}
			onClick={() => stopActor.mutate(name)}
		>
			{stopActor.isLoading ? t('stopping') : t('stop')}
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
