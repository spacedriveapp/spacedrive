import { CheckCircle } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import {
	Themes,
	unitFormatStore,
	useExplorerLayoutStore,
	useThemeStore,
	useUnitFormatStore,
	useZodForm
} from '@sd/client';
import { Button, Divider, Form, Select, SelectOption, SwitchField, z } from '@sd/ui';
import i18n from '~/app/I18n';
import { useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

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

const schema = z.object({
	uiAnimations: z.boolean(),
	syncThemeWithSystem: z.boolean(),
	blurEffects: z.boolean()
});

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

export const Component = () => {
	const { lockAppTheme } = usePlatform();
	const themeStore = useThemeStore();
	const formatStore = useUnitFormatStore();
	const explorerLayout = useExplorerLayoutStore();
	const { t } = useLocale();

	const [selectedTheme, setSelectedTheme] = useState<Theme['themeValue']>(
		themeStore.syncThemeWithSystem === true ? 'system' : themeStore.theme
	);

	const form = useZodForm({
		schema
	});

	const onSubmit = form.handleSubmit(async (data) => {
		console.log({ data });
	});

	useEffect(() => {
		const subscription = form.watch(() => onSubmit());
		return () => {
			subscription.unsubscribe();
		};
	}, [form, onSubmit]);

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

	const hueSliderHandler = (hue: number) => {
		themeStore.hueValue = hue;
		if (themeStore.theme === 'vanilla') {
			document.documentElement.style.setProperty('--light-hue', hue.toString());
		} else if (themeStore.theme === 'dark') {
			document.documentElement.style.setProperty('--dark-hue', hue.toString());
		}
	};

	return (
		<>
			<Form className="relative" form={form} onSubmit={onSubmit}>
				<Heading
					title={t('appearance')}
					description={t('appearance_description')}
					rightArea={
						<div>
							<Button
								disabled={themeStore.hueValue === 235}
								variant={themeStore.hueValue === 235 ? 'outline' : 'accent'}
								size="sm"
								className="flex items-center gap-1"
								onClick={() => {
									hueSliderHandler(235);
								}}
							>
								{t('reset')}
							</Button>
						</div>
					}
				/>
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

				<div className="flex flex-col gap-4">
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
				</div>
			</Form>
			<Divider />
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
			<Divider />
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
			</div>
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
