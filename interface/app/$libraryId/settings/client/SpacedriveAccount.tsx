import { auth, useBridgeQuery } from '@sd/client';
import { Button, Card, Loader } from '@sd/ui';
import { LoginButton } from '~/components/LoginButton';

export function SpacedriveAccount() {
	const authState = auth.useStateSnapshot();

	return (
		<Card className="relative overflow-hidden px-5">
			{authState.status !== 'loggedIn' && (
				<div className="absolute inset-0 z-50 flex items-center justify-center bg-app/75 backdrop-blur-lg">
					{authState.status === 'loading' ? <Loader /> : <LoginButton />}
				</div>
			)}

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
