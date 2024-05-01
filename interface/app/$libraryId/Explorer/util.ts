import dayjs from 'dayjs';
import { type ExplorerItem } from '@sd/client';
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
	en: () => import('dayjs/locale/en.js'),
	en_gb: () => import('dayjs/locale/en-gb.js'),
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
	ja: () => import('dayjs/locale/ja.js')
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
	const DATE_FORMATS = [
		{
			value: 'L',
			label: dayjs().locale(language).format('L')
		},
		{
			value: 'L LT',
			label: dayjs().locale(language).format('L LT')
		},
		// {
		// 	value: 'll',
		// 	label: dayjs().locale(language).format('ll')
		// },
		{
			value: 'LL',
			label: dayjs().locale(language).format('LL')
		},
		// {
		// 	value: 'lll',
		// 	label: dayjs().locale(language).format('lll')
		// },
		{
			value: 'LLL',
			label: dayjs().locale(language).format('LLL')
		},
		{
			value: 'llll',
			label: dayjs().locale(language).format('llll')
		}
	];
	if (language === 'en') {
		const additionalFormats = [
			{
				value: 'DD/MM/YYYY',
				label: dayjs().locale('en').format('DD/MM/YYYY')
			},
			{
				value: 'DD/MM/YYYY HH:mm',
				label: dayjs().locale('en').format('DD/MM/YYYY HH:mm')
			},
			// {
			// 	value: 'D MMM YYYY',
			// 	label: dayjs().locale('en').format('D MMM')
			// },
			{
				value: 'D MMMM YYYY',
				label: dayjs().locale('en').format('D MMMM')
			},
			// {
			// 	value: 'D MMM YYYY HH:mm',
			// 	label: dayjs().locale('en').format('D MMM HH:mm')
			// },
			{
				value: 'D MMMM YYYY HH:mm',
				label: dayjs().locale('en').format('D MMMM HH:mm')
			},
			{
				value: 'ddd, D MMM YYYY HH:mm',
				label: dayjs().locale('en').format('ddd, D MMMM HH:mm')
			}
		];
		return DATE_FORMATS.concat(additionalFormats);
	} else {
		return DATE_FORMATS;
	}
}
