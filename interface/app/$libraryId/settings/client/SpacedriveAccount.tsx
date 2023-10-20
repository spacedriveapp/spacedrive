import { auth, useBridgeQuery } from '@sd/client';
import { Button, Card } from '@sd/ui';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';

export function SpacedriveAccount() {
	return (
		<Card className="relative overflow-hidden !p-5">
			<AuthRequiredOverlay />
			<Account />
		</Card>
	);
}

function Account() {
	const me = useBridgeQuery(['auth.me'], { retry: false });

	return (
		<div className="my-2 flex w-full flex-col">
			<div className="flex items-center justify-between">
				<span className="font-semibold">Spacedrive Account</span>
				<Button variant="gray" onClick={auth.logout}>
					Logout
				</Button>
			</div>
			<hr className="mb-4 mt-2 w-full border-app-line" />
			<span>Logged in as {me.data?.email}</span>
		</div>
	);
}
