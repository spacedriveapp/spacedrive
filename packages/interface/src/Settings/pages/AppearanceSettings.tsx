import clsx from 'clsx';
import {applyTheme} from '../../hooks/useTheme';
import {
	setShellLayout,
	useShellLayout,
	type ShellLayoutMode
} from '../../stores/shellLayoutStore';
import {
	Section,
	SectionCard,
	SectionRows,
	SelectSettingItem,
	SettingsPageContent
} from '../Layout';
import {useAppConfig} from '../useConfig';

interface Theme {
	id: string;
	name: string;
	/** Window fill used by the tile preview */
	surface: string;
	/** True for themes whose preview needs dark-on-light content bars */
	light?: boolean;
}

const THEMES: Theme[] = [
	{id: 'system', name: 'System', surface: 'bg-app'},
	{id: 'light', name: 'Light', surface: 'bg-white', light: true},
	{id: 'dark', name: 'Dark', surface: 'bg-[hsl(235,15%,13%)]'},
	{id: 'midnight', name: 'Midnight', surface: 'bg-[hsl(240,30%,4%)]'},
	{id: 'noir', name: 'Noir', surface: 'bg-[hsl(0,0%,3%)]'},
	{id: 'slate', name: 'Slate', surface: 'bg-[hsl(220,8%,9%)]'},
	{id: 'nord', name: 'Nord', surface: 'bg-[hsl(220,18%,12%)]'},
	{id: 'mocha', name: 'Mocha', surface: 'bg-[hsl(25,18%,10%)]'}
];

const LAYOUTS: {id: ShellLayoutMode; name: string; description: string}[] = [
	{
		id: 'inset',
		name: 'Inset content',
		description: 'Keep files and their inspector together in one sheet.'
	},
	{
		id: 'floating',
		name: 'Floating panels',
		description: 'Float navigation and the inspector over the window.'
	}
];

const LANGUAGES = [
	{value: 'en', label: 'English'},
	{value: 'de', label: 'Deutsch'},
	{value: 'es', label: 'Español'},
	{value: 'fr', label: 'Français'},
	{value: 'it', label: 'Italiano'},
	{value: 'ja', label: '日本語'},
	{value: 'ko', label: '한국어'},
	{value: 'pt', label: 'Português'},
	{value: 'ru', label: 'Русский'},
	{value: 'zh', label: '中文'}
];

export function AppearanceSettings() {
	const {config, update} = useAppConfig();
	const layout = useShellLayout();

	const theme = config?.preferences?.theme ?? 'system';
	const language = config?.preferences?.language ?? 'en';

	const handleThemeChange = (themeId: string) => {
		applyTheme(themeId);
		update({theme: themeId});
	};

	return (
		<SettingsPageContent>
			<Section
				title="Layout"
				description="Choose how navigation and content share your window. Saved on this device."
			>
				<SectionCard padded>
					<div
						className="grid grid-cols-2 gap-4"
						role="group"
						aria-label="Window layout"
					>
						{LAYOUTS.map((option) => (
							<button
								key={option.id}
								type="button"
								aria-pressed={layout === option.id}
								onClick={() => setShellLayout(option.id)}
								className="focus-visible:ring-accent focus-visible:ring-offset-app-box group flex flex-col gap-2 rounded-xl text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
							>
								<div
									aria-hidden="true"
									className={clsx(
										'flex h-28 w-full gap-2 overflow-hidden rounded-lg border-2 p-2 transition-colors',
										layout === option.id
											? 'border-accent'
											: 'border-app-line group-hover:border-app-active',
										option.id === 'inset'
											? 'bg-sidebar'
											: 'bg-app'
									)}
								>
									<div
										className={clsx(
											'flex w-1/4 flex-col gap-1.5 p-2',
											option.id === 'floating' &&
												'bg-sidebar/65 rounded-md'
										)}
									>
										<div className="bg-sidebar-ink/30 mb-1 h-1 w-1/2 rounded-full" />
										{[0, 1, 2, 3].map((row) => (
											<div
												key={row}
												className="bg-sidebar-ink/15 h-1 rounded-full"
											/>
										))}
									</div>
									<div
										className={clsx(
											'flex min-w-0 flex-1',
											option.id === 'inset' &&
												'border-app-line bg-app overflow-hidden rounded-md border'
										)}
									>
										<div className="flex flex-1 flex-col gap-2 p-2">
											<div className="bg-ink/20 h-1 w-2/3 rounded-full" />
											<div className="grid grid-cols-3 gap-1">
												{[0, 1, 2, 3, 4, 5].map(
													(file) => (
														<div
															key={file}
															className="bg-accent/25 h-4 rounded-sm"
														/>
													)
												)}
											</div>
										</div>
										<div
											className={clsx(
												'w-1/4 p-2',
												option.id === 'inset'
													? 'border-app-line border-l'
													: 'bg-sidebar/65 rounded-md'
											)}
										>
											<div className="bg-ink/10 mb-2 h-5 rounded-sm" />
											<div className="bg-ink/20 h-1 rounded-full" />
										</div>
									</div>
								</div>
								<span className="text-ink text-sm font-medium">
									{option.name}
								</span>
								<span className="text-ink-faint text-xs">
									{option.description}
								</span>
							</button>
						))}
					</div>
				</SectionCard>
			</Section>

			<Section
				title="Theme"
				description="Match Spacedrive to your system theme or pick a fixed look."
			>
				<SectionCard padded>
					<div className="grid grid-cols-4 gap-3">
						{THEMES.map((option) => {
							const isSelected = theme === option.id;
							return (
								<button
									key={option.id}
									type="button"
									role="radio"
									aria-checked={isSelected}
									aria-label={`Select theme: ${option.name}`}
									onClick={() => handleThemeChange(option.id)}
									className="focus-visible:ring-accent focus-visible:ring-offset-app-box group flex flex-col items-center gap-2 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
								>
									<div
										className={clsx(
											'aspect-[5/3] w-full overflow-hidden rounded-lg border-2 transition-colors duration-150',
											isSelected
												? 'border-accent'
												: 'border-app-line group-hover:border-app-active'
										)}
									>
										<div
											className={clsx(
												'flex h-full flex-col gap-1 p-2',
												option.surface
											)}
										>
											<div
												className={clsx(
													'h-1 w-3/4 rounded-full',
													option.light
														? 'bg-black/20'
														: 'bg-white/25'
												)}
											/>
											<div
												className={clsx(
													'h-1 w-1/2 rounded-full',
													option.light
														? 'bg-black/10'
														: 'bg-white/10'
												)}
											/>
										</div>
									</div>
									<span
										className={clsx(
											'text-xs font-medium transition-colors',
											isSelected
												? 'text-ink'
												: 'text-ink-faint'
										)}
									>
										{option.name}
									</span>
								</button>
							);
						})}
					</div>
				</SectionCard>
			</Section>

			<Section
				title="Language"
				description="Interface language for Spacedrive."
			>
				<SectionRows>
					<SelectSettingItem
						label="Language"
						description="Select your preferred language"
						value={language}
						onChange={(value) => update({language: value})}
						options={LANGUAGES}
					/>
				</SectionRows>
			</Section>
		</SettingsPageContent>
	);
}
