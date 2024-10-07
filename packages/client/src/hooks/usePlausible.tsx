import Plausible, { PlausibleOptions as PlausibleTrackerOptions } from 'plausible-tracker';
import { useCallback, useEffect, useRef } from 'react';

import { BuildInfo } from '../core';
import { useDebugState } from '../stores/debugState';
import { PlausiblePlatformType, telemetryState, useTelemetryState } from '../stores/telemetryState';

const DOMAIN = 'app.spacedrive.com';
const MOBILE_DOMAIN = 'mobile.spacedrive.com';

let plausibleInstance: ReturnType<typeof Plausible>;

/**
 * This defines all possible options that may be provided by events upon submission.
 *
 * This extends the standard options provided by the `plausible-tracker`
 * package, but also offers some additiional options for custom functionality.
 */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
interface PlausibleOptions extends PlausibleTrackerOptions {
	// the only thing in here before was `telemetryOverride`, but we've removed it
	// keeping this interface around should we need it in the future.
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

type PingEvent = BasePlausibleEvent<'ping'>;

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
	| TagAssignEvent
	| PingEvent;

/**
 * An event information wrapper for internal use only.
 *
 * It means that events can both be logged to the console (if enabled) and submitted to Plausible with ease.
 */
interface PlausibleTrackerEvent {
	eventName: string;
	props: {
		platform: PlausiblePlatformType;
		fullTelemetry: boolean;
		coreVersion: string;
		commitHash: string;
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
	 * Whether or not full telemetry sharing is enabled for the current client.
	 *
	 * It is **crucial** that this is the direct output of `useTelemetryState().shareFullTelemetry`,
	 * regardless of other conditions that may affect whether we share it.
	 */
	shareFullTelemetry: boolean;
	/**
	 * It is **crucial** that this is sourced from the output of `useDebugState()`
	 */
	debugState: {
		enabled: boolean;
		shareFullTelemetry: boolean;
		telemetryLogging: boolean;
	};
	/**
	 * The app's build info
	 */
	buildInfo: BuildInfo | undefined; // TODO(brxken128): ensure this is populated *always*
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
 * * If the user's telemetry preference is not "full", we will only send pings
 * * If the user's telemetry preference is "none", we will never send any telemetry
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
	if (
		// if the user's telemetry preference is not "full", we should only send pings
		props.shareFullTelemetry !== true &&
		event.type !== 'ping'
	)
		return;

	// using a singleton this way instead of instantiating at file eval (first time it's imported)
	// because a user having "none" telemetry preference should mean Plausible never even initalizes
	plausibleInstance ??= Plausible({
		trackLocalhost: true,
		domain: props.platformType === 'mobile' ? MOBILE_DOMAIN : DOMAIN
	});

	const fullEvent: PlausibleTrackerEvent = {
		eventName: event.type,
		props: {
			platform: props.platformType,
			fullTelemetry: props.shareFullTelemetry,
			// we used to fall back to '0.1.0' here, but we should never report an actual version number if we don't know
			coreVersion: props.buildInfo?.version ?? 'unknown',
			commitHash: props.buildInfo?.commit ?? 'unknown',
			debug: debugState.enabled
		},
		options: {
			deviceWidth: props.screenWidth ?? window.screen.width,
			referrer: '',
			// by default do not track current URL, if it's provided in plausibleOptions, that will be sent
			url: '',
			...('plausibleOptions' in event ? event.plausibleOptions : undefined)
		},
		callback: debugState.telemetryLogging
			? () => {
					const { callback: _, ...event } = fullEvent;
					console.log(event);
				}
			: undefined
	};

	plausibleInstance.trackEvent(
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
 * @remarks
 * If any of the following conditions are met, this will return and no data will be submitted:
 *
 * * If the app is in debug/development mode
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
export const usePlausibleEvent = (): ((props: EventSubmissionCallbackProps) => Promise<void>) => {
	const telemetryState = useTelemetryState();

	const debugState = useDebugState();
	const previousEvent = useRef({} as BasePlausibleEvent<string>);

	const sendPlausibleEvent = useCallback(
		async (props: EventSubmissionCallbackProps) => {
			if (previousEvent.current === props.event) return;
			else previousEvent.current = props.event;

			submitPlausibleEvent({
				debugState,
				shareFullTelemetry: telemetryState.telemetryLevelPreference === 'full',
				platformType: telemetryState.platform,
				buildInfo: telemetryState.buildInfo,
				...props
			});
		},
		[debugState, telemetryState]
	);

	if (telemetryState.telemetryLevelPreference === 'none') return async (...args: any[]) => {};

	return sendPlausibleEvent;
};

export interface PlausibleMonitorProps {
	/**
	 * This should be sanitized, containing no user-specific information.
	 *
	 * User-specific values should be replaced with their identifiers.
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
export const usePlausiblePageViewMonitor = ({ currentPath }: PlausibleMonitorProps) => {
	const plausibleEvent = usePlausibleEvent();

	useEffect(() => {
		plausibleEvent({
			event: {
				type: 'pageview',
				plausibleOptions: { url: currentPath }
			}
		});
	}, [currentPath, plausibleEvent]);
};

/**
 * A Plausible Analytics `ping` monitoring hook. It watches the router's current
 * path, and sends events if a change in the path is detected.
 *
 * This should be included next to the {@link usePlausiblePageViewMonitor}.
 *
 * For desktop/web, we use this hook in the `$libraryId` layout and it covers the
 * entire app (excluding onboarding, which should not be monitored).
 *
 * @remarks
 * This will submit an 'ping' event, independently of what the currernt telemetry
 * sharing settings are (minimum or full).
 *
 */
export const usePlausiblePingMonitor = ({ currentPath }: PlausibleMonitorProps) => {
	const plausibleEvent = usePlausibleEvent();

	useEffect(() => {
		plausibleEvent({
			event: {
				type: 'ping'
			}
		});
	}, [currentPath, plausibleEvent]);
};

/**
 * Initializes the `platform` and `buildInfo` properties on `telemetryState` so they can be used
 * by Plausible if it's enabled.
 */
export const configureAnalyticsProperties = ({
	platformType,
	buildInfo
}: {
	platformType: PlausiblePlatformType;
	buildInfo: BuildInfo | undefined;
}) => {
	telemetryState.platform = platformType;
	telemetryState.buildInfo = buildInfo;
	return;
};
