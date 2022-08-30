interface StyleState {
	active: string[];
	hover: string[];
	normal: string[];
}

interface Variant {
	base: string;
	light: StyleState;
	dark: StyleState;
}

function tw(variant: Variant): string {
	return `${variant.base} ${variant.light}`;
}

const variants: Record<string, string> = {
	default: tw({
		base: 'shadow-sm',
		light: {
			normal: ['bg-gray-50', 'border-gray-100', 'text-gray-700'],
			hover: ['bg-gray-100', 'border-gray-200', 'text-gray-900'],
			active: ['bg-gray-50', 'border-gray-200', 'text-gray-600']
		},
		dark: {
			normal: ['bg-gray-800 ', 'border-gray-100', ' text-gray-200'],
			active: ['bg-gray-700 ', 'border-gray-200 ', 'text-white'],
			hover: ['bg-gray-700 ', 'border-gray-600 ', 'text-white']
		}
	})
};
