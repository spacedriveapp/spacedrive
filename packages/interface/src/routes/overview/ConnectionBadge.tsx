import {WifiHigh, WifiSlash} from '@phosphor-icons/react';
import {Tooltip} from '@spacedrive/primitives';
import clsx from 'clsx';
import type React from 'react';
import type {
	ConnectionBadgeState,
	ConnectionMethod
} from './connectionBadgeState';
import {getConnectionBadgeState} from './connectionBadgeState';

interface ConnectionBadgeConfig {
	label: string;
	description: string;
	icon?: React.ComponentType<{className?: string}>;
	color?: string;
}

interface ConnectionBadgeProps {
	method?: ConnectionMethod | null;
	online: boolean;
	current: boolean;
	icon?: React.ComponentType<{className?: string}>;
	color?: string;
}

const configs: Record<ConnectionBadgeState, ConnectionBadgeConfig> = {
	LocalNetwork: {
		label: 'Local',
		description: 'Connected via local network',
		icon: WifiHigh,
		color: 'bg-green-500'
	},
	DirectInternet: {
		label: 'Direct',
		description: 'Connected directly via internet',
		color: 'bg-blue-500'
	},
	RelayProxy: {
		label: 'Relay',
		description: 'Connected via relay proxy',
		color: 'bg-yellow-500'
	},
	Offline: {
		label: 'Offline',
		description: 'Device is currently offline',
		icon: WifiSlash,
		color: 'bg-ink-dull'
	},
	Current: {
		label: 'This device',
		description: 'This is your current device'
	},
	Connecting: {
		label: 'Connecting',
		description: 'Determining how this device is connected',
		color: 'bg-ink-dull'
	}
};

export function ConnectionBadge({
	method,
	online,
	current,
	icon: customIcon,
	color: customColor
}: ConnectionBadgeProps) {
	const state = getConnectionBadgeState({method, online, current});
	const config = configs[state];
	const Icon = customIcon || config.icon || null;
	const dotColor = customColor || config.color || 'bg-ink-dull';

	return (
		<Tooltip label={config.description}>
			<div className="flex items-center gap-1.5">
				{Icon ? (
					<Icon className="size-3" />
				) : (
					!current && (
						<div
							className={clsx('size-2 rounded-full', dotColor)}
						/>
					)
				)}
				<span className="text-ink-dull text-xs font-medium">
					{config.label}
				</span>
			</div>
		</Tooltip>
	);
}
