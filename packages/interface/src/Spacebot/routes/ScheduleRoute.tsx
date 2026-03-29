const COLOR_GROUPS = [
	{
		title: 'Accent',
		swatches: [
			{name: 'accent', className: 'bg-accent'},
			{name: 'accent-faint', className: 'bg-accent-faint'},
			{name: 'accent-deep', className: 'bg-accent-deep'},
		],
	},
	{
		title: 'Ink',
		swatches: [
			{name: 'ink', className: 'bg-ink'},
			{name: 'ink-dull', className: 'bg-ink-dull'},
			{name: 'ink-faint', className: 'bg-ink-faint'},
		],
	},
	{
		title: 'App',
		swatches: [
			{name: 'app', className: 'bg-app'},
			{name: 'app-box', className: 'bg-app-box'},
			{name: 'app-darkBox', className: 'bg-app-darkBox'},
			{name: 'app-darkerBox', className: 'bg-app-darkerBox'},
			{name: 'app-lightBox', className: 'bg-app-lightBox'},
			{name: 'app-overlay', className: 'bg-app-overlay'},
			{name: 'app-input', className: 'bg-app-input'},
			{name: 'app-focus', className: 'bg-app-focus'},
			{name: 'app-line', className: 'bg-app-line'},
			{name: 'app-divider', className: 'bg-app-divider'},
			{name: 'app-button', className: 'bg-app-button'},
			{name: 'app-hover', className: 'bg-app-hover'},
			{name: 'app-selected', className: 'bg-app-selected'},
			{name: 'app-selectedItem', className: 'bg-app-selectedItem'},
			{name: 'app-active', className: 'bg-app-active'},
			{name: 'app-frame', className: 'bg-app-frame'},
			{name: 'app-slider', className: 'bg-app-slider'},
		],
	},
	{
		title: 'Sidebar',
		swatches: [
			{name: 'sidebar', className: 'bg-sidebar'},
			{name: 'sidebar-box', className: 'bg-sidebar-box'},
			{name: 'sidebar-line', className: 'bg-sidebar-line'},
			{name: 'sidebar-ink', className: 'bg-sidebar-ink'},
			{name: 'sidebar-inkDull', className: 'bg-sidebar-inkDull'},
			{name: 'sidebar-inkFaint', className: 'bg-sidebar-inkFaint'},
			{name: 'sidebar-divider', className: 'bg-sidebar-divider'},
			{name: 'sidebar-button', className: 'bg-sidebar-button'},
			{name: 'sidebar-selected', className: 'bg-sidebar-selected'},
			{name: 'sidebar-shade', className: 'bg-sidebar-shade'},
		],
	},
	{
		title: 'Menu',
		swatches: [
			{name: 'menu', className: 'bg-menu'},
			{name: 'menu-line', className: 'bg-menu-line'},
			{name: 'menu-hover', className: 'bg-menu-hover'},
			{name: 'menu-selected', className: 'bg-menu-selected'},
			{name: 'menu-shade', className: 'bg-menu-shade'},
			{name: 'menu-ink', className: 'bg-menu-ink'},
			{name: 'menu-faint', className: 'bg-menu-faint'},
		],
	},
	{
		title: 'Legacy',
		swatches: [
			{name: 'primary', className: 'bg-primary'},
			{name: 'primary-400', className: 'bg-primary-400'},
			{name: 'primary-500', className: 'bg-primary-500'},
			{name: 'gray-400', className: 'bg-gray-400'},
			{name: 'gray-500', className: 'bg-gray-500'},
			{name: 'gray-700', className: 'bg-gray-700'},
			{name: 'gray-900', className: 'bg-gray-900'},
		],
	},
];

function ColorSwatch({name, className}: {name: string; className: string}) {
	return (
		<div className="space-y-2">
			<div className={`aspect-square w-full rounded-2xl border border-app-line ${className}`} />
			<div className="text-ink-dull text-xs leading-tight">{name}</div>
		</div>
	);
}

export function ScheduleRoute() {
	return (
		<div className="border-app-line bg-app-box/90 mx-auto max-h-[calc(100vh-9rem)] w-full max-w-6xl overflow-y-auto rounded-[28px] border p-6 text-left shadow-[0_30px_80px_rgba(0,0,0,0.25)] backdrop-blur-xl">
			<h1 className="text-ink text-3xl font-semibold">Schedule</h1>
			<p className="text-ink-dull mt-2 text-sm">
				Schedule management UI coming soon. For now this page shows the full token palette in-context.
			</p>

			<div className="mt-8 space-y-8">
				{COLOR_GROUPS.map((group) => (
					<section key={group.title} className="space-y-3">
						<h2 className="text-ink text-sm font-medium">{group.title}</h2>
						<div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-7">
							{group.swatches.map((swatch) => (
								<ColorSwatch key={swatch.name} name={swatch.name} className={swatch.className} />
							))}
						</div>
					</section>
				))}
			</div>
		</div>
	);
}
