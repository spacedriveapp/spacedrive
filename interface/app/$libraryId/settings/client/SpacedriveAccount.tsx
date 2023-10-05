import { useBridgeQuery } from '@sd/client';
import { Button, Card, Loader } from '@sd/ui';
import { LoginButton } from '~/components/LoginButton';
import { useAuthContext } from '~/contexts/auth';

export function SpacedriveAccount() {
	const auth = useAuthContext();

	return (
		<Card className="relative overflow-hidden px-5">
			{auth.state !== 'loggedIn' && (
				<div className="absolute inset-0 z-50 flex items-center justify-center bg-app/75 backdrop-blur-lg">
					{auth.state === 'loading' ? <Loader /> : <LoginButton />}
				</div>
			)}

			<Account />
		</Card>
	);
}

function Account() {
	const auth = useAuthContext();
	const me = useBridgeQuery(['auth.me'], { retry: false });

	return (
		<div className="my-2 flex w-full flex-col">
			<div className="flex items-center justify-between">
				<span className="font-semibold">Spacedrive Account</span>
				<Button
					variant="gray"
					onClick={() => auth.logout?.()}
					disabled={auth.logoutLoading}
				>
					Logout
				</Button>
			</div>
			<hr className="mb-4 mt-2 w-full border-app-line" />
			<span>Logged in as {me.data?.email}</span>
		</div>
	);
}
