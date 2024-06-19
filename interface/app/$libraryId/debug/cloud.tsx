import { CheckCircle, XCircle } from '@phosphor-icons/react';
import {
	CloudInstance,
	CloudLibrary,
	HardwareModel,
	auth,
	useLibraryContext,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/client';
import { Button, Card, Loader, tw } from '@sd/ui';
import { Suspense, useMemo } from 'react';
import { Icon } from '~/components';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';
import { LoginButton } from '~/components/LoginButton';
import { useLocale, useRouteTitle } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

export const Component = () => {
	useRouteTitle('Cloud');

	const authState = auth.useStateSnapshot();

	const authSensitiveChild = () => {
		if (authState.status === 'loggedIn') return <Authenticated />;
		if (authState.status === 'notLoggedIn' || authState.status === 'loggingIn')
			return (
				<div className="flex size-full items-center justify-center">
					<Card className="flex flex-col gap-4 !p-6">
						<p>To access cloud related features, please login</p>
						<LoginButton />
					</Card>
				</div>
			);

		return null;
	};

	return <div className="flex size-full flex-col items-start p-4">{authSensitiveChild()}</div>;
};

const DataBox = tw.div`max-w-[300px] rounded-md border border-app-line/50 bg-app-lightBox/20 p-2`;
const Count = tw.div`min-w-[20px] flex h-[20px] px-1 items-center justify-center rounded-full border border-app-button/40 text-[9px]`;

// million-ignore
function Authenticated() {
	const { library } = useLibraryContext();
	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { suspense: true, retry: false });
	const createLibrary = useLibraryMutation(['cloud.library.create']);
	const { t } = useLocale();

	const thisInstance = useMemo(() => {
		if (!cloudLibrary.data) return undefined;
		return cloudLibrary.data.instances.find(
			(instance) => instance.uuid === library.instance_id
		);
	}, [cloudLibrary.data, library.instance_id]);

	return (
		<Suspense
			fallback={
				<div className="flex size-full items-center justify-center">
					<Loader />
				</div>
			}
		>
			{cloudLibrary.data ? (
				<div className="flex flex-col items-start gap-10">
					<Library thisInstance={thisInstance} cloudLibrary={cloudLibrary.data} />
					{thisInstance && <ThisInstance instance={thisInstance} />}
					<Instances instances={cloudLibrary.data.instances} />
				</div>
			) : (
				<div className="relative flex size-full flex-col items-center justify-center">
					<AuthRequiredOverlay />
					<Button
						disabled={createLibrary.isLoading}
						variant="accent"
						onClick={() => {
							createLibrary.mutateAsync(null);
						}}
					>
						{createLibrary.isLoading
							? t('connecting_library_to_cloud')
							: t('connect_library_to_cloud')}
					</Button>
				</div>
			)}
		</Suspense>
	);
}

// million-ignore
const Instances = ({ instances }: { instances: CloudInstance[] }) => {
	const { library } = useLibraryContext();
	const filteredInstances = instances.filter((instance) => instance.uuid !== library.instance_id);
	return (
		<div className="flex flex-col gap-3">
			<div className="flex flex-row items-center gap-3">
				<p className="text-medium font-bold">Instances</p>
				<Count>{filteredInstances.length}</Count>
			</div>
			<div className="flex flex-row flex-wrap gap-2">
				{filteredInstances.map((instance) => (
					<Card
						key={instance.id}
						className="flex-col items-center gap-4 bg-app-box/50 !p-5"
					>
						<div className="flex flex-col items-center gap-2">
							<Icon
								name={
									hardwareModelToIcon(
										instance.metadata.device_model as HardwareModel
									) as any
								}
								size={70}
							/>
							<p className="max-w-[250px] truncate text-xs font-medium">
								{instance.metadata.name}
							</p>
						</div>
						<div className="flex flex-col gap-1.5">
							<DataBox>
								<p className="truncate text-xs font-medium">
									Id:{' '}
									<span className="font-normal text-ink-dull">{instance.id}</span>
								</p>
							</DataBox>
							<DataBox>
								<p className="truncate text-xs font-medium">
									UUID:{' '}
									<span className="font-normal text-ink-dull">
										{instance.uuid}
									</span>
								</p>
							</DataBox>
							<DataBox>
								<p className="truncate text-xs font-medium">
									Public Key:{' '}
									<span className="font-normal text-ink-dull">
										{instance.identity}
									</span>
								</p>
							</DataBox>
						</div>
					</Card>
				))}
			</div>
		</div>
	);
};

interface LibraryProps {
	cloudLibrary: CloudLibrary;
	thisInstance: CloudInstance | undefined;
}

// million-ignore
const Library = ({ thisInstance, cloudLibrary }: LibraryProps) => {
	const syncLibrary = useLibraryMutation(['cloud.library.sync']);
	return (
		<div className="flex flex-col gap-3">
			<p className="text-medium font-bold">Library</p>
			<Card className="flex-row items-center gap-6 !px-2">
				<p className="font-medium">
					Name: <span className="font-normal text-ink-dull">{cloudLibrary.name}</span>
				</p>
				<Button
					disabled={syncLibrary.isLoading || thisInstance !== undefined}
					variant={thisInstance === undefined ? 'accent' : 'gray'}
					className="flex flex-row items-center gap-1 !text-ink"
					onClick={() => syncLibrary.mutateAsync(null)}
				>
					{thisInstance === undefined ? (
						<XCircle weight="fill" size={15} className="text-red-400" />
					) : (
						<CheckCircle weight="fill" size={15} className="text-green-400" />
					)}
					{thisInstance === undefined ? 'Sync Library' : 'Library synced'}
				</Button>
			</Card>
		</div>
	);
};

interface ThisInstanceProps {
	instance: CloudInstance;
}

// million-ignore
const ThisInstance = ({ instance }: ThisInstanceProps) => {
	return (
		<div className="flex flex-col gap-3">
			<p className="text-medium font-bold">This Instance</p>
			<Card className="flex-col items-center gap-4 bg-app-box/50 !p-5">
				<div className="flex flex-col items-center gap-2">
					<Icon
						name={
							hardwareModelToIcon(
								instance.metadata.device_model as HardwareModel
							) as any
						}
						size={70}
					/>
					<p className="max-w-[160px] truncate text-xs font-medium">
						{instance.metadata.name}
					</p>
				</div>
				<div className="flex flex-col gap-1.5">
					<DataBox>
						<p className="truncate text-xs font-medium">
							Id: <span className="font-normal text-ink-dull">{instance.id}</span>
						</p>
					</DataBox>
					<DataBox>
						<p className="truncate text-xs font-medium">
							UUID: <span className="font-normal text-ink-dull">{instance.uuid}</span>
						</p>
					</DataBox>
					<DataBox>
						<p className="truncate text-xs font-medium">
							Public Key:{' '}
							<span className="font-normal text-ink-dull">{instance.identity}</span>
						</p>
					</DataBox>
				</div>
			</Card>
		</div>
	);
};
