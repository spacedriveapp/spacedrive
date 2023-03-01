import Plausible from 'plausible-tracker';
import { useEffect, useMemo, useRef } from 'react';
import { useCurrentLibraryId, useCurrentTelemetrySharing } from './useClientContext';

/**
 * These props are required by the `PlausibleTracker`.
 */
export interface PlausibleProps {
	currentPath: string; // useLocation().pathname. must have leading `/` - (e.g. `/settings/keys`)
	platformType: 'web' | 'tauri' | 'mobile'; // desktop should use `usePlatform().platform` (mobile can statically set this)
}

/**
 * These rules will be compared to a path using `string.startsWith()`.
 *
 * If it's a match, the path will be replaced with the target path.
 */
const TrackerReplaceRules: [string, string][] = [
	['/location/', '/explorer/locations'],
	['/tag/', '/explorer/tags']
];

const UuidRegex = new RegExp(
	'[a-f0-9]{8}-?[a-f0-9]{4}-?4[a-f0-9]{3}-?[89ab][a-f0-9]{3}-?[a-f0-9]{12}'
);

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
	const previousPath = useRef('');
	const currentLibraryId = useCurrentLibraryId();
	const shareTelemetry = useCurrentTelemetrySharing();

	const { trackPageview } = useMemo(
		() =>
			Plausible({
				trackLocalhost: true,
				domain: `${props.platformType == 'tauri' ? 'desktop' : props.platformType}.spacedrive.com`
			}),
		[props.platformType]
	);

	// This sanitises the current path, so that our analytics aren't flooded with unique (UUID-filled) records.
	// It also replaces certain routes - see the `TrackerReplaceRules` for more info.
	const path = useMemo(() => {
		let path =
			currentLibraryId !== null
				? props.currentPath.replace(`/${currentLibraryId}`, '')
				: props.currentPath;

		TrackerReplaceRules.every((e, i) => {
			if (!path.startsWith(e[0])) return true;

			path = e[1];
			return false;
		});

		return path;
	}, [props.currentPath, currentLibraryId]);

	// This actually sends the request and does the tracking.
	const track = async () => {
		trackPageview({
			url: path,
			deviceWidth: window.screen.width
		});
	};

	useEffect(() => {
		if (shareTelemetry !== true) return;
		if (previousPath.current === path) return;
		if (UuidRegex.test(path)) return;

		previousPath.current = path;
		track();

		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [path, currentLibraryId, shareTelemetry]);

	return <></>;
};
