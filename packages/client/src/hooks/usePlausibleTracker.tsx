import Plausible from 'plausible-tracker';
import { useEffect, useMemo, useRef } from 'react';
import { useDebugState } from '../stores';
import { useCurrentLibraryId, useCurrentTelemetrySharing } from './useClientContext';

/**
 * These props are required by the `PlausibleTracker`
 *
 * Usage:
 *
 * ```ts
 * 	<PlausibleTracker currentPath={useLocation().pathname} platformType={usePlatform().platform} />
 * ```
 *
 */
export interface PlausibleProps {
	currentPath: string; // must have leading `/` (e.g. `/settings/keys`)
	platformType: 'web' | 'tauri' | 'mobile'; // web/tauri should should set this via `usePlatform().platform`
}

const UuidRegex = '[a-f0-9]{8}-?[a-f0-9]{4}-?4[a-f0-9]{3}-?[89ab][a-f0-9]{3}-?[a-f0-9]{12}';

/**
 * These rules will be matched as regular expressions with `.replace()`.
 *
 * If it's a match, the expression will be replaced with the target value.
 */
const TrackerReplaceRules: [RegExp, string][] = [
	[RegExp(`/${UuidRegex}`), ''],
	[RegExp('/location/[0-9]+'), '/explorer/locations'],
	[RegExp('/tag/[0-9]+'), '/explorer/tags']
];

const { trackEvent } = Plausible({
	trackLocalhost: true,
	domain: `app.spacedrive.com`
});

/**
 * Adds a Plausible Analytics tracker which monitors the router's location and sends data accordingly.
 *
 * Ideally this should be added to layouts extremely early in the app - as early as they viably can be.
 *
 * More instances of this component will both worsen code readability and force `useMemo` updates
 * every time layouts are switched between.
 *
 * No data will be sent if telemetry is disabled via the library configuration (`useCurrentTelemetrySharing()`).
 *
 * Usage:
 *
 * ```ts
 * 	<PlausibleTracker currentPath={useLocation().pathname} platformType={usePlatform().platform} />
 * ```
 */
export const PlausibleTracker = (props: PlausibleProps) => {
	const currentLibraryId = useCurrentLibraryId();
	const shareTelemetry = useCurrentTelemetrySharing();
	const debugState = useDebugState();
	const previousPath = useRef('');

	let path = props.currentPath;

	// This sanitises the current path, so that our analytics aren't flooded with unique (UUID-filled) records.
	// It also replaces certain routes - see the `TrackerReplaceRules` for more info.
	TrackerReplaceRules.forEach((e, i) => (path = path.replace(e[0], e[1])));

	// This actually sends the network request/does the tracking
	const track = async () => {
		trackEvent(
			'pageview',
			{
				props: {
					app: props.platformType == 'tauri' ? 'desktop' : props.platformType,
					version: '0.0.0'
				}
			},
			{ url: path, deviceWidth: window.screen.width }
		);
	};

	// Check that the following prerequisites are met:
	// telemetry sharing is explicitly enabled
	// the current path is not the same as the previous path
	useEffect(() => {
		if (debugState.enabled === true) return;
		if (shareTelemetry !== true) return;
		if (path === previousPath.current) return;

		previousPath.current = path;
		track();

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [path, currentLibraryId, shareTelemetry]);

	return <></>;
};
