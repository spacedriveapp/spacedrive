import { useQueryClient } from '@tanstack/react-query';
import { useBridgeMutation, useBridgeQuery } from '@sd/client';
import { Button, Card, Loader } from '@sd/ui';
import { LoginButton } from '~/components/LoginButton';

export function SpacedriveAccount() {
	const user = useBridgeQuery(['auth.me'], {
		// If the backend returns un unauthorised error we don't want to retry
		retry: false
	});

	const logout = useBridgeMutation(['auth.logout']);

	const queryClient = useQueryClient();

	return (
		<Card className="relative overflow-hidden px-5">
			{!user.data && (
				<div className="absolute inset-0 z-50 flex items-center justify-center bg-app/75 backdrop-blur-lg">
					{!user.isFetchedAfterMount ? (
						<Loader />
					) : (
						<LoginButton onLogin={user.refetch} />
					)}
				</div>
			)}

			<div className="my-2 flex w-full flex-col">
				<div className="flex items-center justify-between">
					<span className="font-semibold">Spacedrive Account</span>
					<Button
						variant="gray"
						onClick={async () => {
							await logout.mutateAsync(undefined);
							// this sucks but oh well :)
							queryClient.setQueryData(['auth.me'], null);
						}}
						disabled={logout.isLoading || !user.data}
					>
						Logout
					</Button>
				</div>
				<hr className="mb-4 mt-2 w-full border-app-line" />
				<span>Logged in as {user.data?.email}</span>
			</div>
		</Card>
	);
}
