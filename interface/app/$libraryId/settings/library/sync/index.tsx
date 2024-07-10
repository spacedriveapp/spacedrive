import { inferSubscriptionResult } from '@oscartbeaumont-sd/rspc-client';
import { CheckCircle, XCircle } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useMemo, useState } from 'react';
import {
	auth,
	CloudInstance,
	CloudLibrary,
	Procedures,
	useFeatureFlag,
	useLibraryContext,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription,
	useZodForm
} from '@sd/client';
import {
	Button,
	Card,
	cx,
	Dialog,
	dialogManager,
	Select,
	SelectOption,
	Switch,
	useDialog,
	UseDialogProps,
	z
} from '@sd/ui';
import { useLocale } from '~/hooks';

import { Heading } from '../../Layout';
import Setting from '../../Setting';

export enum SyncMethod {
	P2P,
	Cloud
}

export enum EnabledSystems {
	Ingest,
	CloudSend,
	CloudReceive,
	CloudIngest
}

export interface SyncOptions {
	sync_method: SyncMethod;
	enabled_systems: EnabledSystems;
}

const SYS_NAMES = {
	[EnabledSystems.Ingest]: 'Sync Ingest',
	[EnabledSystems.CloudSend]: 'Cloud Sync Sender',
	[EnabledSystems.CloudReceive]: 'Cloud Sync Receiver',
	[EnabledSystems.CloudIngest]: 'Cloud Sync Ingest'
};

export const Component = () => {
	const { t } = useLocale();
	const authState = auth.useStateSnapshot();

	const syncEnabled = useLibraryQuery(['sync.enabled']);
	const [localSyncEnabled, setLocalSyncEnabled] = useState(syncEnabled.data || false);

	useEffect(() => {
		setLocalSyncEnabled(syncEnabled.data || false);
	}, [syncEnabled.data]);

	const allSystemsEnabled = useMemo(() => {
		if (!syncEnabled.data) return false;
		return Object.values(syncEnabled.data).every((enabled) => enabled);
	}, [syncEnabled.data]);

	const backfillSync = useLibraryMutation(['sync.backfill'], {
		onSuccess: async () => {
			await syncEnabled.refetch();
		}
	});

	const [data, setData] = useState<inferSubscriptionResult<Procedures, 'library.actors'>>({});

	useLibrarySubscription(['library.actors'], { onData: setData });

	const cloudSync = useFeatureFlag('cloudSync');

	function createBackfillDialog() {
		dialogManager.create((dialogProps) => (
			<SyncBackfillDialog onEnabled={() => syncEnabled.refetch()} {...dialogProps} />
		));
	}

	return (
		<>
			<Heading title={t('sync')} description={t('sync_description')} />
			<Setting mini title={t('enable_sync')} description={t('enable_sync_description')}>
				<div>
					{/* <Button
						className="text-nowrap"
						variant="accent"
						onClick={createBackfillDialog}
						disabled={backfillSync.isLoading}
					>
						{t('enable_sync')}
					</Button> */}
					<Switch
						checked={localSyncEnabled}
						onChange={() => !syncEnabled && createBackfillDialog()}
					/>
				</div>
			</Setting>
			<hr className="border-1 my-2 w-full border-gray-200 opacity-10" />
			<Setting mini title={t('fuck around')} description={t('findout')}>
				<div>
					<Switch />
				</div>
			</Setting>
			<div
				className={cx(
					'flex flex-col gap-4',
					syncEnabled.data === false && 'pointer-events-none  opacity-35'
				)}
			>
				<Setting
					mini
					title={
						<>
							{t('ingester')}
							<OnlineIndicator
								online={data[SYS_NAMES[EnabledSystems.Ingest]] ?? false}
							/>
						</>
					}
					description={t('injester_description')}
				>
					<div>
						<SyncServiceSwitch
							enabled={data[SYS_NAMES[EnabledSystems.Ingest]] ?? false}
							name={SYS_NAMES[EnabledSystems.Ingest]}
						/>
					</div>
				</Setting>
				<Setting
					mini
					title={t('Sync Method')}
					description={t('Method to sync library data')}
				>
					<div>
						<div className="flex h-[30px] gap-2 ">
							<Select
								value="cloud"
								containerClassName="h-[30px] whitespace-nowrap"
								onChange={() => {}}
							>
								<SelectOption value="cloud">Spacedrive Cloud</SelectOption>
								<SelectOption value="p2p">P2P</SelectOption>
							</Select>
						</div>
					</div>
				</Setting>
				<hr className="border-1 my-2 w-full border-gray-200 opacity-10" />

				{cloudSync && <CloudSync data={data} />}
			</div>
		</>
	);
};

interface LibraryProps {
	cloudLibrary: CloudLibrary;
	thisInstance: CloudInstance | undefined;
}

const LibrarySyncedStatus = ({ thisInstance, cloudLibrary }: LibraryProps) => {
	const syncLibrary = useLibraryMutation(['cloud.library.sync']);
	return (
		<Card className="my-3 flex-row items-center justify-between gap-6 !pr-2">
			<p className="font-medium">
				Name:{' '}
				<span className="truncate font-normal text-ink-dull">{cloudLibrary.name}</span>
			</p>
			<p className="font-medium">
				UUID:{' '}
				<span className="truncate font-normal text-ink-dull">{cloudLibrary.uuid}</span>
			</p>

			<p className="font-medium ">
				Last Synced: <span className="truncate font-normal text-ink-dull">Just now</span>
			</p>
			<div className="flex shrink-0 flex-row items-center gap-2">
				<Button className="shrink-0" variant="gray">
					Sync Now
				</Button>
				<Button
					disabled={syncLibrary.isLoading || thisInstance !== undefined}
					variant={thisInstance === undefined ? 'accent' : 'gray'}
					className="flex shrink-0 flex-row items-center gap-1 !text-ink"
					onClick={() => syncLibrary.mutateAsync(null)}
				>
					{thisInstance === undefined ? (
						<XCircle weight="fill" size={15} className="text-red-400" />
					) : (
						<CheckCircle weight="fill" size={15} className="text-green-400" />
					)}
					{thisInstance === undefined ? 'Sync Library' : 'Library synced'}
				</Button>
			</div>
		</Card>
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
	const { library } = useLibraryContext();

	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { suspense: true, retry: false });

	const thisInstance = useMemo(() => {
		if (!cloudLibrary.data) return undefined;
		return cloudLibrary.data.instances.find(
			(instance) => instance.uuid === library.instance_id
		);
	}, [cloudLibrary.data, library.instance_id]);

	return (
		<>
			<div>
				<h1 className="mb-0.5 text-lg font-bold text-ink">{t('cloud_sync')}</h1>
				<p className="text-sm text-ink-faint">{t('cloud_sync_description')}</p>
			</div>
			{cloudLibrary.data && (
				<LibrarySyncedStatus thisInstance={thisInstance} cloudLibrary={cloudLibrary.data} />
			)}
			<Setting
				mini
				title={
					<>
						{t('sender')}{' '}
						<OnlineIndicator
							online={data[SYS_NAMES[EnabledSystems.CloudSend]] ?? false}
						/>
					</>
				}
				description={t('sender_description')}
			>
				<div>
					<SyncServiceSwitch
						enabled={data[SYS_NAMES[EnabledSystems.CloudSend]] ?? false}
						name={SYS_NAMES[EnabledSystems.CloudSend]}
					/>
				</div>
			</Setting>
			<Setting
				mini
				title={
					<>
						{t('receiver')}
						<OnlineIndicator
							online={data[SYS_NAMES[EnabledSystems.CloudReceive]] ?? false}
						/>
					</>
				}
				description={t('receiver_description')}
			>
				<div>
					<SyncServiceSwitch
						enabled={data[SYS_NAMES[EnabledSystems.CloudReceive]] ?? false}
						name={SYS_NAMES[EnabledSystems.CloudReceive]}
					/>
				</div>
			</Setting>
			<Setting
				mini
				title={
					<>
						{t('ingester')}
						<OnlineIndicator
							online={data[SYS_NAMES[EnabledSystems.CloudIngest]] ?? false}
						/>
					</>
				}
				description={t('ingester_description')}
			>
				<div>
					<SyncServiceSwitch
						enabled={data[SYS_NAMES[EnabledSystems.CloudIngest]] ?? false}
						name={SYS_NAMES[EnabledSystems.CloudIngest]}
					/>
				</div>
			</Setting>
		</>
	);
}

function SyncServiceSwitch({ enabled, name }: { enabled: boolean; name: string }) {
	const { t } = useLocale();
	const startActor = useLibraryMutation(['library.startActor']);
	const stopActor = useLibraryMutation(['library.stopActor']);
	// local state to handle the switch
	const [localEnabled, setLocalEnabled] = useState(enabled);

	const toggle = () => {
		// if we are loading, don't do anything
		if (startActor.isLoading || stopActor.isLoading) return;
		// enabled is the true state, so we decide what to do based on that
		if (enabled) {
			stopActor.mutate(name);
		} else {
			startActor.mutate(name);
		}
		setLocalEnabled(!localEnabled);
	};

	useEffect(() => {
		setLocalEnabled(enabled);
	}, [enabled]);

	return <Switch checked={localEnabled} onClick={toggle} />;
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
