import { ScreenHeading } from '@sd/ui';
import { useLocale } from '~/hooks';

export const Component = () => {
	const { t } = useLocale();
	return <ScreenHeading>{t('people')}</ScreenHeading>;
};
