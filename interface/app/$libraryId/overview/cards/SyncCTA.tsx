import { Cloud } from '@phosphor-icons/react';
import { useNavigate } from 'react-router';
import { Button } from '@sd/ui';
import { useLocale } from '~/hooks';

const SyncCTA = () => {
	const navigate = useNavigate();
	const { t } = useLocale();

	return (
		<div className="flex h-full flex-col items-center justify-center gap-4 text-center">
			<div className="-mt-4 rounded-full bg-app-selected/50 p-4">
				<Cloud className="size-8 text-accent" weight="fill" />
			</div>
			<div className="flex flex-col gap-1">
				<span className="text-lg font-semibold">Enable Cloud Sync</span>
				<span className="text-sm text-ink-dull">
					Keep your files in sync across all your devices
				</span>
			</div>
			<Button
				variant="accent"
				className="mt-0"
				onClick={() => navigate('/settings/library/sync')}
			>
				Set up sync
			</Button>
		</div>
	);
};

export default SyncCTA;
