import { CheckCircle } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useState } from 'react';
import { Themes, useExplorerLayoutStore, useThemeStore, useUnitFormatStore } from '@sd/client';
import { Select, SelectOption } from '@sd/ui';
import i18n from '~/app/I18n';
import { useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { generateLocaleDateFormats } from '../../Explorer/util';
import HorizontalScroll from '../../overview/Layout/HorizontalScroll';
import { Heading } from '../Layout';
import Setting from '../Setting';

type Theme = {
	insideColor: string;
	outsideColor: string;
	textColor: string;
	border: string;
	themeName: string;
	themeValue: Themes | 'system';
};

type ThemeProps = Theme & { isSelected?: boolean; className?: string };

const themes: Theme[] = [
	{
		insideColor: 'bg-white',
		outsideColor: 'bg-[#F0F0F0]',
		textColor: 'text-black',
		border: 'border border-[#E6E6E6]',
		themeName: i18n.t('light'),
		themeValue: 'vanilla'
	},
	{
		insideColor: 'bg-[#1C1D26]', //Not using theme color because we want it to stay the same color when theme is toggled
		outsideColor: 'bg-black',
		textColor: 'text-white',
		border: 'border border-[#323342]',
		themeName: i18n.t('dark'),
		themeValue: 'dark'
	},
	{
		insideColor: '',
		outsideColor: '',
		textColor: 'text-white',
		border: 'border border-[#323342]',
		themeName: i18n.t('system'),
		themeValue: 'system'
	}
];

// Unsorted list of languages available in the app.
const LANGUAGE_OPTIONS = [
	{ value: 'ar', label: 'عربي' },
	{ value: 'en', label: 'English' },
	{ value: 'de', label: 'Deutsch' },
	{ value: 'es', label: 'Español' },
	{ value: 'cs', label: 'Czech' },
	{ value: 'fr', label: 'Français' },
	{ value: 'tr', label: 'Türkçe' },
	{ value: 'nl', label: 'Nederlands' },
	{ value: 'be', label: 'Беларуская' },
	{ value: 'ru', label: 'Русский' },
	{ value: 'zh_CN', label: '中文（简体）' },
	{ value: 'zh_TW', label: '中文（繁體）' },
	{ value: 'it', label: 'Italiano' },
	{ value: 'ja', label: '日本語' },
	{ value: 'uk', label: 'Українська' }
];

// Sort the languages by their label
LANGUAGE_OPTIONS.sort((a, b) => a.label.localeCompare(b.label));

export const Component = () => {
	const { lockAppTheme } = usePlatform();
	const themeStore = useThemeStore();
	const formatStore = useUnitFormatStore();
	const explorerLayout = useExplorerLayoutStore();

	const [dateFormats, setDateFormats] = useState(
		generateLocaleDateFormats(i18n.resolvedLanguage || i18n.language || 'en')
	);

	const { t, dateFormat, setDateFormat } = useLocale();

	const [selectedTheme, setSelectedTheme] = useState<Theme['themeValue']>(
		themeStore.syncThemeWithSystem === true ? 'system' : themeStore.theme
	);

	const themeSelectHandler = (theme: Theme['themeValue']) => {
		setSelectedTheme(theme);
		if (theme === 'system') {
			lockAppTheme?.('Auto');
			themeStore.syncThemeWithSystem = true;
		} else if (theme === 'vanilla') {
			themeStore.syncThemeWithSystem = false;
			themeStore.theme = theme;
			document.documentElement.classList.add('vanilla-theme');
		} else if (theme === 'dark') {
			themeStore.syncThemeWithSystem = false;
			themeStore.theme = theme;
			document.documentElement.classList.remove('vanilla-theme');
		}
	};

	return (
		<>
			<Heading title={t('appearance')} description={t('appearance_description')} />
			<HorizontalScroll className="!mb-5 mt-4 !pl-0">
				<div className="flex gap-3 md:w-[300px] lg:w-full">
					{themes.map((theme, i) => {
						return (
							<div
								onClick={() => themeSelectHandler(theme.themeValue)}
								className={clsx(
									'shrink-0',
									selectedTheme !== theme.themeValue &&
										'opacity-70 transition-all duration-300 hover:opacity-100'
								)}
								key={i}
							>
								{theme.themeValue === 'system' ? (
									<SystemTheme
										{...theme}
										isSelected={selectedTheme === 'system'}
									/>
								) : (
									<Theme
										{...theme}
										isSelected={selectedTheme === theme.themeValue}
									/>
								)}
							</div>
						);
					})}
				</div>
			</HorizontalScroll>

			{/* {themeStore.theme === 'dark' && (
					<Setting mini title="Theme hue value" description="Change the hue of the theme">
						<div className="mr-3 w-full max-w-[200px] justify-between gap-5">
							<div className="w-full">
								<Slider
									value={[themeStore.hueValue ?? 235]}
									onValueChange={(val) => hueSliderHandler(val[0] ?? 235)}
									min={0}
									max={359}
									step={1}
									defaultValue={[235]}
								/>
								<p className="text-xs text-center text-ink-faint">
									{themeStore.hueValue}
								</p>
							</div>
						</div>
					</Setting>
				)} */}

			{/* <div className="flex flex-col gap-4">
					<Setting
						mini
						title={t('ui_animations')}
						className="opacity-30"
						description={t('ui_animations_description')}
					>
						<SwitchField
							disabled
							{...form.register('uiAnimations')}
							className="m-2 ml-4"
						/>
					</Setting>

					<Setting
						mini
						title={t('blur_effects')}
						className="opacity-30"
						description={t('blur_effects_description')}
					>
						<SwitchField
							disabled
							{...form.register('blurEffects')}
							className="m-2 ml-4"
						/>
					</Setting>
				</div> */}
			{/* Language Settings */}
			<Setting mini title={t('language')} description={t('language_description')}>
				<div className="flex h-[30px] gap-2">
					<Select
						value={i18n.resolvedLanguage || i18n.language || 'en'}
						onChange={(e) => {
							// if previous language was English, set date formatting for default value
							if ((i18n.resolvedLanguage || i18n.language) === 'en') {
								localStorage.setItem('sd-date-format', 'LL');
								setDateFormat('LL');
							}

							// add "i18nextLng" key to localStorage and set it to the selected language
							localStorage.setItem('i18nextLng', e);
							i18n.changeLanguage(e);

							setDateFormats(generateLocaleDateFormats(e));
						}}
						containerClassName="h-[30px] whitespace-nowrap"
					>
						{LANGUAGE_OPTIONS.map((lang, key) => (
							<SelectOption key={key} value={lang.value}>
								{lang.label}
							</SelectOption>
						))}
					</Select>
				</div>
			</Setting>
			{/* Date Formatting Settings */}
			<Setting
				mini
				title={t('date_time_format')}
				description={t('date_time_format_description')}
			>
				<div className="flex h-[30px] gap-2">
					<Select
						value={dateFormat}
						onChange={(e) => {
							// add "dateFormat" key to localStorage and set it as default date format
							localStorage.setItem('sd-date-format', e);
							setDateFormat(e);
						}}
						containerClassName="h-[30px] whitespace-nowrap"
					>
						{dateFormats.map((format, key) => (
							<SelectOption key={key} value={format.value}>
								{format.label}
							</SelectOption>
						))}
					</Select>
				</div>
			</Setting>

			{/* <Divider /> */}
			<div className="flex flex-col gap-4">
				<h1 className="mb-3 text-lg font-bold text-ink">{t('default_settings')}</h1>
				<Setting
					mini
					title={t('explorer_view')}
					description={t('change_view_setting_description')}
				>
					<Select
						onChange={(v) => (explorerLayout.defaultView = v)}
						value={explorerLayout.defaultView}
					>
						<SelectOption value="grid">{t('grid_view')}</SelectOption>
						<SelectOption value="list">{t('list_view')}</SelectOption>
						<SelectOption value="media">{t('media_view')}</SelectOption>
					</Select>
				</Setting>
			</div>
			{/* <Divider />
			<div className="flex flex-col gap-4">
				<h1 className="mb-3 text-lg font-bold text-ink">{t('display_formats')}</h1>
				<Setting mini title={t('coordinates')}>
					<Select
						onChange={(e) => (unitFormatStore.coordinatesFormat = e)}
						value={formatStore.coordinatesFormat}
					>
						<SelectOption value="dms">DMS</SelectOption>
						<SelectOption value="dd">Decimal</SelectOption>
					</Select>
				</Setting>

				<Setting mini title={t('distance')}>
					<Select
						onChange={(e) => (unitFormatStore.distanceFormat = e)}
						value={formatStore.distanceFormat}
					>
						<SelectOption value="km">{t('kilometers')}</SelectOption>
						<SelectOption value="miles">{t('miles')}</SelectOption>
					</Select>
				</Setting>

				<Setting mini title={t('temperature')}>
					<Select
						onChange={(e) => (unitFormatStore.temperatureFormat = e)}
						value={formatStore.temperatureFormat}
					>
						<SelectOption value="celsius">{t('celcius')}</SelectOption>
						<SelectOption value="fahrenheit">{t('fahrenheit')}</SelectOption>
					</Select>
				</Setting>
			</div> */}
		</>
	);
};

function Theme(props: ThemeProps) {
	return (
		<div className="w-[150px]">
			<div
				className={clsx(
					props.outsideColor,
					props.border,
					props.textColor,
					props.className,
					'relative h-[90px] overflow-hidden rounded-lg'
				)}
			>
				<div
					className={clsx(
						props.insideColor,
						props.border,
						'absolute bottom-[-10px] right-[-2px] h-[70px] w-[118px] rounded-tl-lg p-3'
					)}
				>
					<p>Aa</p>
				</div>
				{props.isSelected && (
					<CheckCircle
						weight="fill"
						size={24}
						className={`absolute bottom-1.5 right-1.5 z-10 text-accent`}
					/>
				)}
			</div>
			<p className="my-3 text-center text-sm">{props.themeName}</p>
		</div>
	);
}

function SystemTheme(props: ThemeProps) {
	return (
		<div className="w-[150px]">
			<div className="relative flex h-[90px]">
				<div className="relative h-full w-1/2 grow overflow-hidden rounded-l-lg bg-black">
					<Theme className="rounded-r-none" {...themes[1]!} />
				</div>
				<div className={clsx('relative h-full w-1/2 grow overflow-hidden rounded-r-lg')}>
					<Theme className="rounded-l-none" {...themes[0]!} />
				</div>
				{props.isSelected && (
					<CheckCircle
						weight="fill"
						size={24}
						className={`absolute bottom-1.5 right-1.5 z-10 text-accent`}
					/>
				)}
			</div>
			<p className="mt-3 text-center text-sm">{props.themeName}</p>
		</div>
	);
}
