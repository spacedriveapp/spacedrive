import { Cube, Envelope, User } from '@phosphor-icons/react';
import { Collection, Drive_Light, Folder, Laptop } from '@sd/assets/icons';
import { memo, useEffect, useMemo, useState } from 'react';
import { auth, byteSize, useBridgeQuery, useDiscoveredPeers, useLibraryQuery } from '@sd/client';
import { Button, Card } from '@sd/ui';
import { TruncatedText } from '~/components';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';
import { useCounter } from '~/hooks';

import { Heading } from '../Layout';

export const Component = () => {
	const me = useBridgeQuery(['auth.me'], { retry: false });
	const authStore = auth.useStateSnapshot();
	return (
		<>
			<Heading
				rightArea={
					<>
						{authStore.status === 'loggedIn' && (
							<div className="flex-row space-x-2">
								<Button variant="accent" size="sm" onClick={auth.logout}>
									Logout
								</Button>
							</div>
						)}
					</>
				}
				title="Your account"
				description="Spacedrive account and information."
			/>
			<div className="flex flex-col justify-between gap-5 xl:flex-row">
				<Profile authStore={authStore} email={me.data?.email} />
				<Usage />
			</div>
		</>
	);
};

const Profile = ({ email, authStore }: { email?: string; authStore: { status: string } }) => {
	const emailName = authStore.status === 'loggedIn' ? email?.split('@')[0] : 'guest user';
	return (
		<Card className="relative flex w-full flex-col items-center justify-center !p-6 xl:max-w-[300px]">
			<AuthRequiredOverlay />
			<div
				className="flex h-[90px] w-[90px] items-center justify-center
	 rounded-full border border-app-line bg-app-input"
			>
				<User weight="fill" className="mx-auto text-4xl text-ink-faint" />
			</div>
			<h1 className="mx-auto mt-3 text-lg">
				Welcome <span className="font-bold">{emailName},</span>
			</h1>
			<div className="mx-auto mt-4 flex w-full flex-col gap-2">
				<Card className="w-full items-center justify-start gap-1 bg-app-input !px-2">
					<div className="w-[20px]">
						<Envelope weight="fill" width={20} />
					</div>
					<TruncatedText>
						{authStore.status === 'loggedIn' ? email : 'guestuser@outlook.com'}
					</TruncatedText>
				</Card>
				<Card className="flex w-full items-center justify-start gap-1 bg-app-input !px-2">
					<div className="w-[20px]">
						<Cube width={20} weight="fill" />
					</div>
					<p>Free</p>
				</Card>
			</div>
		</Card>
	);
};

const Usage = memo(() => {
	const stats = useLibraryQuery(['library.statistics'], {
		refetchOnWindowFocus: false,
		initialData: { total_bytes_capacity: '0', library_db_size: '0' }
	});
	const locations = useLibraryQuery(['locations.list'], {
		refetchOnWindowFocus: false
	});
	const discoveredPeers = useDiscoveredPeers();
	const info = useMemo(() => {
		const tb_capacity = byteSize(stats.data?.total_bytes_capacity);
		const library_db_size = byteSize(stats.data?.library_db_size);
		const data: {
			icon: string;
			title?: string;
			numberTitle?: number;
			titleCount?: number;
			unit?: string;
			sub: string;
			dataLength?: number;
		}[] = [
			{
				icon: Folder,
				title: 'Locations',
				titleCount: locations.data?.length ?? 0,
				sub: 'indexed directories'
			},
			{
				icon: Laptop,
				title: discoveredPeers.size >= 0 ? 'Devices' : 'Device',
				titleCount: discoveredPeers.size ?? 0,
				sub: 'in your network'
			},
			{
				icon: Drive_Light,
				numberTitle: tb_capacity.value,
				sub: 'Total capacity',
				unit: tb_capacity.unit
			},
			{
				icon: Collection,
				numberTitle: library_db_size.value,
				sub: 'Library size',
				unit: library_db_size.unit
			}
		];
		return data;
	}, [locations, discoveredPeers, stats]);

	return (
		<Card className="flex w-full flex-col justify-center !p-6">
			<h1 className="text-lg font-bold">Usage & Hardware</h1>
			<div className="mt-5 grid grid-cols-1 justify-center gap-2 lg:grid-cols-2">
				{info.map((i, index) => (
					<UsageCard
						key={index}
						icon={i.icon}
						title={i.title as string}
						numberTitle={i.numberTitle}
						titleCount={i.titleCount as number}
						statsLoading={stats.isLoading}
						unit={i.unit}
						sub={i.sub}
					/>
				))}
			</div>
		</Card>
	);
});

interface Props {
	icon: string;
	title: string;
	titleCount?: number;
	numberTitle?: number;
	statsLoading: boolean;
	unit?: string;
	sub: string;
}

let mounted = false;
const UsageCard = memo(
	({ icon, title, titleCount, numberTitle, unit, sub, statsLoading }: Props) => {
		const [isMounted] = useState(mounted);
		const sizeCount = useCounter({
			name: title,
			end: Number(numberTitle ? numberTitle : titleCount),
			duration: isMounted ? 0 : 1,
			precision: numberTitle ? 1 : 0,
			saveState: false
		});
		useEffect(() => {
			if (!statsLoading) mounted = true;
		});

		return (
			<Card className="h-[90px] w-full bg-app-input py-4">
				<div className="flex w-full items-center justify-center gap-3">
					<img src={icon} className="w-10" />
					<div className="w-full max-w-[120px]">
						<h1 className="text-lg font-medium">
							{typeof titleCount === 'number' && (
								<span className="mr-1 text-ink-dull">{sizeCount}</span>
							)}
							{numberTitle && sizeCount}
							{title}
							{unit && (
								<span className="ml-1 text-[16px] font-normal text-ink-dull">
									{unit}
								</span>
							)}
						</h1>
						<p className="text-sm text-ink-faint">{sub}</p>
					</div>
				</div>
			</Card>
		);
	}
);
