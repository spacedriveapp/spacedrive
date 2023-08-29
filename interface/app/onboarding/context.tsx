import { useQueryClient } from '@tanstack/react-query';
import { createContext, useContext } from 'react';
import { useNavigate } from 'react-router';
import {
	currentLibraryCache,
	getOnboardingStore,
	resetOnboardingStore,
	telemetryStore,
	useBridgeMutation,
	useCachedLibraries,
	useMultiZodForm,
	useOnboardingStore,
	usePlausibleEvent
} from '@sd/client';
import { RadioGroupField, z } from '@sd/ui';

export const OnboardingContext = createContext<ReturnType<typeof useContextValue> | null>(null);

// Hook for generating the value to put into `OnboardingContext.Provider`,
// having it separate removes the need for a dedicated context type.
export const useContextValue = () => {
	const libraries = useCachedLibraries();
	const library =
		libraries.data?.find((l) => l.uuid === currentLibraryCache.id) || libraries.data?.[0];

	const form = useFormState();

	return {
		...form,
		libraries,
		library
	};
};

export const shareTelemetry = RadioGroupField.options([
	z.literal('share-telemetry'),
	z.literal('minimal-telemetry')
]).details({
	'share-telemetry': {
		heading: 'Share anonymous usage',
		description:
			'Share completely anonymous telemetry data to help the developers improve the app'
	},
	'minimal-telemetry': {
		heading: 'Share the bare minimum',
		description: 'Only share that I am an active user of Spacedrive and a few technical bits'
	}
});

const schemas = {
	'new-library': z.object({
		name: z.string().min(1, 'Name is required').regex(/[\S]/g).trim()
	}),
	'privacy': z.object({
		shareTelemetry: shareTelemetry.schema
	})
};

const useFormState = () => {
	const obStore = useOnboardingStore();

	const { handleSubmit, ...forms } = useMultiZodForm({
		schemas,
		defaultValues: {
			'new-library': obStore.data?.['new-library'] ?? undefined,
			'privacy': obStore.data?.privacy ?? {
				shareTelemetry: 'share-telemetry'
			}
		},
		onData: (data) => (getOnboardingStore().data = data)
	});

	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const submitPlausibleEvent = usePlausibleEvent();

	const createLibrary = useBridgeMutation('library.create');

	const submit = handleSubmit(
		async (data) => {
			navigate('./creating-library', { replace: true });

			// opted to place this here as users could change their mind before library creation/onboarding finalization
			// it feels more fitting to configure it here (once)
			telemetryStore.shareFullTelemetry = data.privacy.shareTelemetry === 'share-telemetry';

			try {
				// show creation screen for a bit for smoothness
				const [library] = await Promise.all([
					createLibrary.mutateAsync({
						name: data['new-library'].name
					}),
					new Promise((res) => setTimeout(res, 500))
				]);

				queryClient.setQueryData(['library.list'], (libraries: any) => [
					...(libraries ?? []),
					library
				]);

				if (telemetryStore.shareFullTelemetry) {
					submitPlausibleEvent({ event: { type: 'libraryCreate' } });
				}

				resetOnboardingStore();
				navigate(`/${library.uuid}/overview`, { replace: true });
			} catch (e) {
				if (e instanceof Error) {
					alert(`Failed to create library. Error: ${e.message}`);
				}
				navigate('./privacy');
			}
		},
		(key) => navigate(`./${key}`)
	);

	return { submit, forms };
};

export const useOnboardingContext = () => {
	const ctx = useContext(OnboardingContext);

	if (!ctx)
		throw new Error('useOnboardingContext must be used within OnboardingContext.Provider');

	return ctx;
};
