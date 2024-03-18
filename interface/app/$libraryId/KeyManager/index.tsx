import { Button, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

export function KeyManager() {
	const { t } = useLocale();
	// const isUnlocked = useLibraryQuery(['keys.isUnlocked']);
	// const isSetup = useLibraryQuery(['keys.isSetup']);

	return (
		<div className="flex h-full max-w-[300px] flex-col">
			<div className="flex w-full flex-col items-center p-4">
				<Icon name="Keys" size={56} />
				<span className="text-lg font-bold">{t('key_manager')}</span>
				<span className="mt-2 text-center text-ink-dull">
					{t('key_manager_description')}
				</span>
				<Tooltip className="w-full" label="Coming soon!">
					<Button disabled className="mt-4 w-full" variant="accent">
						{t('setup')}
					</Button>
				</Tooltip>
			</div>
		</div>
	);
}
