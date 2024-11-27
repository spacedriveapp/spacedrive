import dayjs from 'dayjs';
import { type ExplorerItem } from '@sd/client';
import i18n from '~/app/I18n';
import { ExplorerParamsSchema } from '~/app/route-schemas';
import { useZodSearchParams } from '~/hooks';

export function useExplorerSearchParams() {
	return useZodSearchParams(ExplorerParamsSchema);
}

export const pubIdToString = (pub_id: number[]) =>
	pub_id.map((b) => b.toString(16).padStart(2, '0')).join('');

export const uniqueId = (item: ExplorerItem | { pub_id: number[] }) => {
	if ('pub_id' in item) return pubIdToString(item.pub_id);

	const { type } = item;

	switch (type) {
		case 'NonIndexedPath':
			return item.item.path;
		case 'SpacedropPeer':
		case 'Label':
			return item.item.name;
		default:
			return pubIdToString(item.item.pub_id);
	}
};

export function getItemId(index: number, items: ExplorerItem[]) {
	const item = items[index];
	return item ? uniqueId(item) : undefined;
}

export function getItemData(index: number, items: ExplorerItem[]) {
	return items[index];
}

const dayjsLocales: Record<string, any> = {
	ar: () => import('dayjs/locale/ar.js'),
	en: () => import('dayjs/locale/en.js'),
	de: () => import('dayjs/locale/de.js'),
	es: () => import('dayjs/locale/es.js'),
	fr: () => import('dayjs/locale/fr.js'),
	tr: () => import('dayjs/locale/tr.js'),
	nl: () => import('dayjs/locale/nl.js'),
	be: () => import('dayjs/locale/be.js'),
	ru: () => import('dayjs/locale/ru.js'),
	zh_CN: () => import('dayjs/locale/zh-cn.js'),
	zh_TW: () => import('dayjs/locale/zh-tw.js'),
	it: () => import('dayjs/locale/it.js'),
	ja: () => import('dayjs/locale/ja.js'),
	uk: () => import('dayjs/locale/uk.js')
};

export function loadDayjsLocale(language: string) {
	if (dayjsLocales[language]) {
		dayjsLocales[language]()
			.then(() => {
				language = language.replace('_', '-');
				dayjs.locale(language);
			})
			.catch((error: any) => {
				console.error(`Failed to load ${language} locale:`, error);
				// Optionally set a default locale here
				dayjs.locale('en');
			});
	} else {
		console.warn(`Locale for ${language} not available, falling back to default.`);
		dayjs.locale('en');
	}
}

// Generate list of localized formats available in the app
export function generateLocaleDateFormats(language: string) {
	language = language.replace('_', '-');
	// this is a good example date because:
	// - day segment is greater than 12, no confusion about the month
	// - month segment is below 10, no confusion about zero-padding
	// - hours segment is below 10, no confusion about zero-padding
	// - is a Monday, just a good day of week for examples
	const defaultDate = '2024-01-15 08:51';
	const DATE_FORMATS = [
		{
			value: 'L',
			label: dayjs(defaultDate).locale(language).format('L')
		},
		{
			value: 'L, LT',
			label: dayjs(defaultDate).locale(language).format('L, LT')
		},
		{
			value: 'll',
			label: dayjs(defaultDate).locale(language).format('ll')
		},
		{
			value: 'LL',
			label: dayjs(defaultDate).locale(language).format('LL')
		},
		{
			value: 'lll',
			label: dayjs(defaultDate).locale(language).format('lll')
		},
		{
			value: 'LLL',
			label: dayjs(defaultDate).locale(language).format('LLL')
		},
		{
			value: 'llll',
			label: dayjs(defaultDate).locale(language).format('llll')
		}
	];
	if (language === 'en') {
		const additionalFormats = [
			{
				value: 'DD/MM/YYYY',
				label: dayjs(defaultDate).locale('en').format('DD/MM/YYYY')
			},
			{
				value: 'DD/MM/YYYY HH:mm',
				label: dayjs(defaultDate).locale('en').format('DD/MM/YYYY HH:mm')
			},
			{
				value: 'D MMM, YYYY',
				label: dayjs(defaultDate).locale('en').format('D MMM, YYYY')
			},
			{
				value: 'D MMMM, YYYY',
				label: dayjs(defaultDate).locale('en').format('D MMMM, YYYY')
			},
			{
				value: 'D MMM, YYYY HH:mm',
				label: dayjs(defaultDate).locale('en').format('D MMM, YYYY HH:mm')
			},
			{
				value: 'D MMMM, YYYY HH:mm',
				label: dayjs(defaultDate).locale('en').format('D MMMM, YYYY HH:mm')
			},
			{
				value: 'ddd, D MMM, YYYY HH:mm',
				label: dayjs(defaultDate).locale('en').format('ddd, D MMMM, YYYY HH:mm')
			}
		];
		return DATE_FORMATS.concat(additionalFormats);
	} else {
		return DATE_FORMATS;
	}
}

const kinds: Record<string, string> = {
	Unknown: `${i18n.t('unknown')}`,
	Document: `${i18n.t('document')}`,
	Folder: `${i18n.t('folder')}`,
	Text: `${i18n.t('text')}`,
	Package: `${i18n.t('package')}`,
	Image: `${i18n.t('image')}`,
	Audio: `${i18n.t('audio')}`,
	Video: `${i18n.t('video')}`,
	Archive: `${i18n.t('archive')}`,
	Executable: `${i18n.t('executable')}`,
	Alias: `${i18n.t('alias')}`,
	Encrypted: `${i18n.t('encrypted')}`,
	Key: `${i18n.t('key')}`,
	Link: `${i18n.t('link')}`,
	WebPageArchive: `${i18n.t('web_page_archive')}`,
	Widget: `${i18n.t('widget')}`,
	Album: `${i18n.t('album')}`,
	Collection: `${i18n.t('collection')}`,
	Font: `${i18n.t('font')}`,
	Mesh: `${i18n.t('mesh')}`,
	Code: `${i18n.t('code')}`,
	Database: `${i18n.t('database')}`,
	Book: `${i18n.t('book')}`,
	Config: `${i18n.t('config')}`,
	Dotfile: `${i18n.t('dotfile')}`,
	Screenshot: `${i18n.t('screenshot')}`,
	Label: `${i18n.t('label')}`
};

export function translateKindName(kindName: string): string {
	if (kinds[kindName]) {
		try {
			const kind = kinds[kindName] as string;
			return kind;
		} catch (error) {
			console.error(`Failed to load ${kindName} translation:`, error);
			return kindName;
		}
	} else {
		console.warn(`Translation for ${kindName} not available, falling back to passed value.`);
		return kindName;
	}
}

export function fetchAccessToken(): string {
	const accessToken: string =
		JSON.parse(window.localStorage.getItem('frontendCookies') ?? '[]')
			.find((cookie: string) => cookie.startsWith('st-access-token'))
			?.split('=')[1]
			.split(';')[0] || '';
	return accessToken;
}
