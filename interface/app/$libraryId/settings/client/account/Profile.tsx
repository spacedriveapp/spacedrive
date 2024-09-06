import { Envelope } from '@phosphor-icons/react';
import { Card } from '@sd/ui';
import { TruncatedText } from '~/components';
import { AuthRequiredOverlay } from '~/components/AuthRequiredOverlay';

const Profile = ({ email }: { email?: string }) => {
	const emailName = email?.split('@')[0];
	const capitalizedEmailName = (emailName?.charAt(0).toUpperCase() ?? '') + emailName?.slice(1);

	return (
		<Card className="relative flex w-full flex-col items-center justify-center !p-0 lg:max-w-[320px]">
			{/* <AuthRequiredOverlay /> */}
			<div className="p-3">
				<h1 className="mx-auto mt-3 text-lg">
					Welcome <span className="font-bold">{capitalizedEmailName},</span>
				</h1>
				<div className="mx-auto mt-4 flex w-full flex-col gap-2">
					<Card className="w-full items-center justify-start gap-1 bg-app-input !px-2">
						<div className="w-[20px]">
							<Envelope weight="fill" width={20} />
						</div>
						<TruncatedText>{email}</TruncatedText>
					</Card>
				</div>
			</div>
		</Card>
	);
};

export default Profile;
