import dayjs from 'dayjs';
import localizedFormat from 'dayjs/plugin/localizedFormat';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { loadDayjsLocale } from '~/app/$libraryId/Explorer/util';

dayjs.extend(localizedFormat);

export const useLocale = (namespace: Parameters<typeof useTranslation>[0] = 'translation') => {
	const { i18n, t } = useTranslation(namespace);
	const isLocaleReady = Object.keys(i18n).length > 0;
	loadDayjsLocale(i18n.resolvedLanguage || i18n.language || 'en');
	const [dateFormat, setDateFormat] = useState(localStorage.getItem('sd-date-format') || 'LL');

	return {
		i18n,
		t,
		isLocaleReady,
		dateFormat,
		setDateFormat
	};
};
