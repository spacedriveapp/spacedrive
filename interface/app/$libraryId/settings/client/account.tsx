import { Envelope, User } from '@phosphor-icons/react';
import { iconNames } from '@sd/assets/util';
import { auth, useBridgeQuery } from '@sd/client';
import { Button, Card } from '@sd/ui';
import { Icon, TruncatedText } from '~/components';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';

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
				title="Spacedrive Cloud"
				description="Spacedrive is always local first, but we will offer our own optional cloud services in the future. For now, authentication is only used for the Feedback feature, otherwise it is not required."
			/>
			<div className="flex flex-col justify-between gap-5 lg:flex-row">
				<Profile authStore={authStore} email={me.data?.email} />
			</div>
		</>
	);
};

const Profile = ({ email, authStore }: { email?: string; authStore: { status: string } }) => {
	const emailName = authStore.status === 'loggedIn' ? email?.split('@')[0] : 'guest user';
	return (
		<Card className="relative flex w-full flex-col items-center justify-center !p-6 lg:max-w-[320px]">
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
			</div>
		</Card>
	);
};
