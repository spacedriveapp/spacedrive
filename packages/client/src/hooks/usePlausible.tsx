import Plausible, { PlausibleOptions as PlausibleTrackerOptions } from 'plausible-tracker';
import { useCallback, useEffect, useRef } from 'react';
import { PlausiblePlatformType, telemetryStore, useDebugState, useTelemetryState } from '../stores';

/**
 * This should be in sync with the Core's version.
 */
const VERSION = '0.1.0';
const DOMAIN = 'app.spacedrive.com';

const PlausibleProvider = Plausible({
	trackLocalhost: true,
	domain: DOMAIN
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
	 * must be allowed/denied via external means. Currently it is not used by anything,
	 * but probably will be in the future.
	 */
	telemetryOverride?: boolean;
}

/**
 * The base Plausible event, that all other events must be derived
 * from in an effort to keep things type-safe.
 */
type BasePlausibleEventWithOption<T, O extends keyof PlausibleOptions> = {
	type: T;
	plausibleOptions: Required<{
		[K in O]: PlausibleOptions[O];
	}>;
};

type BasePlausibleEventWithoutOption<T> = {
	type: T;
};

export type BasePlausibleEvent<T, O = void> = O extends keyof PlausibleOptions
	? BasePlausibleEventWithOption<T, O>
	: BasePlausibleEventWithoutOption<T>;

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
 * const submitPlausibleEvent = usePlausibleEvent();
 *
 * const createLibrary = useBridgeMutation('library.create', {
 *		onSuccess: (library) => {
 *			submitPlausibleEvent({
 *				event: {
 *					type: 'libraryCreate'
 *				}
 *			});
 *		}
 * });
 * ```
 */
type LibraryCreateEvent = BasePlausibleEvent<'libraryCreate'>;
type LibraryDeleteEvent = BasePlausibleEvent<'libraryDelete'>;

type LocationCreateEvent = BasePlausibleEvent<'locationCreate'>;
type LocationDeleteEvent = BasePlausibleEvent<'locationDelete'>;

type TagCreateEvent = BasePlausibleEvent<'tagCreate'>;
type TagDeleteEvent = BasePlausibleEvent<'tagDelete'>;
type TagAssignEvent = BasePlausibleEvent<'tagAssign'>;

/**
 * All union of available, ready-to-use events.
 *
 * Every possible event must also be added as a "goal" in Plausible's settings (on their site) for the currently active {@link DOMAIN domain}.
 */
type PlausibleEvent =
	| PageViewEvent
	| LibraryCreateEvent
	| LibraryDeleteEvent
	| LocationCreateEvent
	| LocationDeleteEvent
	| TagCreateEvent
	| TagDeleteEvent
	| TagAssignEvent;

/**
 * An event information wrapper for internal use only.
 *
 * It means that events can both be logged to the console (if enabled) and submitted to Plausible with ease.
 */
interface PlausibleTrackerEvent {
	eventName: string;
	props: {
		platform: PlausiblePlatformType;
		version: string;
		debug: boolean;
	};
	options: PlausibleTrackerOptions;
	callback?: () => void;
}

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
	 * @see {@link PlausiblePlatformType}
	 */
	platformType: PlausiblePlatformType;
	/**
	 * An optional screen width. Default is `window.screen.width`
	 */
	screenWidth?: number;
	/**
	 * Whether or not telemetry sharing is enabled for the current client.
	 *
	 * It is **crucial** that this is the direct output of `useTelemetryState().shareTelemetry`,
	 * regardless of other conditions that may affect whether we share it (such as event overrides).
	 */
	shareTelemetry: boolean;
	/**
	 * It is **crucial** that this is sourced from the output of `useDebugState()`
	 */
	debugState: {
		enabled: boolean;
		shareTelemetry: boolean;
		telemetryLogging: boolean;
	};
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
const submitPlausibleEvent = async ({ event, debugState, ...props }: SubmitEventProps) => {
	if (props.platformType === 'unknown') return;
	if (debugState.enabled && debugState.shareTelemetry !== true) return;
	if (
		'plausibleOptions' in event && 'telemetryOverride' in event.plausibleOptions
			? event.plausibleOptions.telemetryOverride !== true
			: props.shareTelemetry !== true
	)
		return;

	const fullEvent: PlausibleTrackerEvent = {
		eventName: event.type,
		props: {
			platform: props.platformType,
			version: VERSION,
			debug: debugState.enabled
		},
		options: {
			deviceWidth: props.screenWidth ?? window.screen.width,
			// referrer: '', // TODO(brxken128): see if we could have this blank to prevent accidental IP logging
			...('plausibleOptions' in event ? event.plausibleOptions : undefined)
		},
		callback: debugState.telemetryLogging
			? () => {
					const { callback: _, ...event } = fullEvent;
					console.log(event);
			  }
			: undefined
	};

	PlausibleProvider.trackEvent(
		fullEvent.eventName,
		{
			props: fullEvent.props,
			callback: fullEvent.callback
		},
		fullEvent.options
	);
};

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
 * Certain events provide functionality to override the clients's telemetry sharing configuration.
 * This is not to ignore the user's choice, but because it should **only** be used in contexts where
 * telemetry sharing must be allowed/denied via external means.
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
 * const submitPlausibleEvent = usePlausibleEvent();
 *
 * const createLibrary = useBridgeMutation('library.create', {
 *		onSuccess: (library) => {
 *			submitPlausibleEvent({
 *				event: {
 *					type: 'libraryCreate'
 *				}
 *			});
 *		}
 * });
 * ```
 */
export const usePlausibleEvent = () => {
	const debugState = useDebugState();
	const telemetryState = useTelemetryState();
	const previousEvent = useRef({} as BasePlausibleEvent<string>);

	return useCallback(
		async (props: EventSubmissionCallbackProps) => {
			if (previousEvent.current === props.event) return;
			else previousEvent.current = props.event;

			submitPlausibleEvent({
				debugState,
				shareTelemetry: telemetryState.shareTelemetry,
				platformType: telemetryState.platform,
				...props
			});
		},
		[debugState, telemetryState]
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
 * PageViewRegexRules.forEach((e) => (path = path.replace(e[0], e[1])));
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
	[RegExp('/tag/[0-9]+'), '/tag'],
	/**
	 * This is for removing location IDs from the current path, when in library settings (e.g. `/settings/library/locations/12`)
	 */
	[RegExp('/locations/[0-9]+'), '/locations']
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
 * If any of the following conditions are met, this will return and no data will be submitted:
 *
 * * If the app is in debug/development mode
 * * If telemetry sharing (sourced from the client configuration) is not true
 *
 * @example
 * ```ts
 *  usePlausiblePageViewMonitor({
 *  	currentPath: useLocation().pathname
 *  });
 * ```
 */
export const usePlausiblePageViewMonitor = ({ currentPath }: PageViewMonitorProps) => {
	const plausibleEvent = usePlausibleEvent();

	let path = currentPath;
	PageViewRegexRules.forEach((e) => (path = path.replace(e[0], e[1])));

	useEffect(() => {
		plausibleEvent({
			event: {
				type: 'pageview',
				plausibleOptions: { url: path }
			}
		});
	}, [path, plausibleEvent]);
};

export const initPlausible = ({ platformType }: { platformType: PlausiblePlatformType }) => {
	telemetryStore.platform = platformType;
	return;
};
