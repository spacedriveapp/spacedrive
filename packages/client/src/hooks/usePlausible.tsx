import Plausible from 'plausible-tracker';
import { PlausibleOptions as PlausibleTrackerOptions } from 'plausible-tracker';
import { useCallback, useEffect } from 'react';
import { useDebugState } from '../stores';
import { useCurrentTelemetrySharing } from './useClientContext';

const Version = '0.1.0';
type PlatformType = 'web' | 'mobile' | 'tauri';

const Domain = 'app.spacedrive.com';

const plausible = Plausible({
	trackLocalhost: true,
	domain: Domain
});

/**
 * This defines all possible options that may be provided by events upon submission.
 *
 * This extends the standard options provided by the `plausible-tracker`
 * package, but also offers some additiional options for custom functionality.
 */
interface PlausibleOptions extends PlausibleTrackerOptions {
	/**
	 * This should **only** be used in contexts where telemetry sharing
	 * must be allowed via external means (such as during onboarding,
	 * where we can't source it from the library configuration).
	 */
	telemetryOverride?: boolean;
}

/**
 * The base Plausible event, that all other events must be derived
 * from in an effort to keep things type-safe.
 */
type BasePlausibleEvent<T, O extends keyof PlausibleOptions> = {
	type: T;
	plausibleOptions: Required<{
		[K in O]: PlausibleOptions[O];
	}>;
};

/**
 * The Plausible `pageview` event.
 *
 * **Do not use this directly. Instead, use the
 * {@link usePlausiblePageViewMonitor `usePlausiblePageViewMonitor`} hook**.
 */
type PageViewEvent = BasePlausibleEvent<'pageview', 'url'>;

/**
 * The custom Plausible `libraryCreate` event.
 *
 * @example
 * ```ts
 * const platform = usePlatform();
 * const createLibraryEvent = usePlausibleEvent({ platformType: platform.platform });
 *
 * const createLibrary = useBridgeMutation('library.create', {
 *		onSuccess: (library) => {
 *			createLibraryEvent({
 *				event: {
 *					type: 'libraryCreate',
 *					plausibleOptions: { telemetryOverride: library.config.shareTelemetry }
 *				}
 *			});
 *		}
 * });
 * ```
 */
type LibraryCreateEvent = BasePlausibleEvent<'libraryCreate', 'telemetryOverride'>;

/**
 * All union of available, ready-to-use events.
 */
type PlausibleEvent = PageViewEvent | LibraryCreateEvent;

interface SubmitEventProps {
	/**
	 * The Plausible event to submit.
	 *
	 * @see {@link PlausibleEvent}
	 */
	event: PlausibleEvent;
	/**
	 *  The current platform type. This should be the output of `usePlatform().platform`
	 *
	 * @see {@link PlatformType}
	 */
	platformType: PlatformType;
	/**
	 * An optional screen width. Default is `window.screen.width`
	 */
	screenWidth?: number;
	/**
	 * Whether or not telemetry sharing is enabled for the current library.
	 *
	 * It is **crucial** that this is the direct output of `useCurrentTelemetrySharing()`,
	 * regardless of other conditions that may affect whether we share it (such as event overrides).
	 */
	shareTelemetry: boolean | null;
	/**
	 * It is **crucial** that this is the direct output of `useDebugState().enabled`
	 */
	debug: boolean;
	/**
	 * A function to be executed if/when the event has been successfully submitted.
	 */
	onSuccess?: () => void;
}

/**
 * This function is for directly submitting events to Plausible.
 *
 * **Avoid using this directly, but if it's necessary then do not misuse this API and only
 * send telemetry when certain that it has been allowed by the user. Always prefer the
 * {@link usePlausibleEvent `usePlausibleEvent`} hook.**
 *
 * @remarks
 * If any of the following conditions are met, this will return and no data will be submitted:
 *
 * * If the app is in debug/development mode
 * * If a telemetry override is present, but it is not true
 * * If no telemetry override is present, and telemetry sharing is not true
 *
 * @privateRemarks
 * Telemetry sharing settings are never matched to `=== false`, but to `!== true` instead.
 * This means we can always guarantee that **nothing** will be sent unless the user
 * explicitly allows it.
 *
 * @see {@link https://plausible.io/docs/custom-event-goals Custom events}
 * @see {@link https://plausible-tracker.netlify.app/#tracking-custom-events-and-goals Tracking custom events}
 */
const submitPlausibleEvent = async (props: SubmitEventProps) => {
	const { event } = props;

	if (props.debug === true) return;
	if (
		'telemetryOverride' in event ? event.telemetryOverride !== true : props.shareTelemetry !== true
	)
		return;

	plausible.trackEvent(
		event.type,
		{
			props: {
				app: props.platformType == 'tauri' ? 'desktop' : props.platformType,
				version: Version
			},
			...props.onSuccess
		},
		{ deviceWidth: props.screenWidth ?? window.screen.width, ...event.plausibleOptions }
	);
};

interface UsePlausibleEventProps {
	/**
	 *  The current platform type. This should be the output of `usePlatform().platform`
	 *
	 * @see {@link PlatformType}
	 */
	platformType: PlatformType;
}

interface EventSubmissionCallbackProps {
	/**
	 * The plausible event to submit.
	 *
	 * @see {@link PlausibleEvent}
	 */
	event: PlausibleEvent;
}

/**
 * A Plausible Analytics event submission hook.
 *
 * The returned callback should only be fired once,
 * in order to prevent our analytics from being flooded.
 *
 * Certain events provide functionality to override the library's telemetry sharing configuration.
 * This is not to ignore the user's choice, but because it should **only** be used in contexts where
 * telemetry sharing must be allowed via external means (such as during onboarding, where we can't
 * source it from the library configuration).
 *
 * @remarks
 * If any of the following conditions are met, this will return and no data will be submitted:
 *
 * * If the app is in debug/development mode
 * * If a telemetry override is present, but it is not true
 * * If no telemetry override is present, and telemetry sharing is not true
 *
 * @returns a callback that, once executed, will submit the desired event
 *
 * @example
 * ```ts
 * const platform = usePlatform();
 * const createLibraryEvent = usePlausibleEvent({ platformType: platform.platform });
 *
 * const createLibrary = useBridgeMutation('library.create', {
 *		onSuccess: (library) => {
 *			createLibraryEvent({
 *				event: {
 *					type: 'libraryCreate',
 *					plausibleOptions: { telemetryOverride: library.config.shareTelemetry }
 *				}
 *			});
 *		}
 * });
 * ```
 */
export const usePlausibleEvent = (props: UsePlausibleEventProps) => {
	const { platformType } = props;
	const debug = useDebugState().enabled;
	const shareTelemetry = useCurrentTelemetrySharing();

	return useCallback(
		async (props: EventSubmissionCallbackProps) => {
			submitPlausibleEvent({ debug, shareTelemetry, platformType, ...props });
		},
		[debug, platformType, shareTelemetry]
	);
};

/**
 * These rules will be matched as regular expressions via `.replace()`
 * in a `forEach` loop.
 *
 * If a rule matches, the expression will be replaced with the target value.
 *
 * @example
 * ```ts
 * let path = "/ed0c715c-d095-4f6a-b83c-1d0b25cc89e7/location/1";
 * PageViewRegexRules.forEach((e, i) => (path = path.replace(e[0], e[1])));
 * assert(path === "/location");
 * ```
 */
const PageViewRegexRules: [RegExp, string][] = [
	/**
	 * This is for removing the library UUID from the current path
	 */
	[RegExp('/[a-f0-9]{8}-?[a-f0-9]{4}-?4[a-f0-9]{3}-?[89ab][a-f0-9]{3}-?[a-f0-9]{12}'), ''],
	/**
	 * This is for removing location IDs from the current path
	 */
	[RegExp('/location/[0-9]+'), '/location'],
	/**
	 * This is for removing tag IDs from the current path
	 */
	[RegExp('/tag/[0-9]+'), '/tag']
];

export interface PageViewMonitorProps {
	/**
	 * This should be unsanitized, and should still contain
	 * all dynamic parameters (such as the library UUID).
	 *
	 * Ideally, this should be the output of `useLocation().pathname`
	 *
	 * @see {@link PageViewRegexRules} for sanitization
	 */
	currentPath: string;
	/**
	 *  The current platform type. This should be the output of `usePlatform().platform`
	 *
	 * @see {@link PlatformType}
	 */
	platformType: PlatformType;
}

/**
 * A Plausible Analytics `pageview` monitoring hook. It watches the router's current
 * path, and sends events if a change in the path is detected.
 *
 * Ideally this should be added to the app extremely early - the sooner the better.
 * This means we don't need as many hooks to cover the same amount of routes.
 *
 * For desktop/web, we use this hook in the `$libraryId` layout and it covers the
 * entire app (excluding onboarding, which should not be monitored).
 *
 * @remarks
 * Do **not** attempt to track pages that do not have a `ClientContext` - it's useless.
 *
 * If any of the following conditions are met, this will return and no data will be submitted:
 *
 * * If the app is in debug/development mode
 * * If telemetry sharing (sourced from the library configuration) is not true
 *
 * @example
 * ```ts
 *  usePlausiblePageViewMonitor({
 *  	currentPath: useLocation().pathname,
 *  	platformType: usePlatform().platform
 *  });
 * ```
 */
export const usePlausiblePageViewMonitor = (props: PageViewMonitorProps) => {
	const plausibleEvent = usePlausibleEvent({ platformType: props.platformType });

	let path = props.currentPath;
	PageViewRegexRules.forEach((e, i) => (path = path.replace(e[0], e[1])));

	useEffect(() => {
		plausibleEvent({
			event: {
				type: 'pageview',
				plausibleOptions: { url: path }
			}
		});
	}, [path, plausibleEvent]);
};
