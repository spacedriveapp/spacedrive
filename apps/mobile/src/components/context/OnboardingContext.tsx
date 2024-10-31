import { useNavigation } from '@react-navigation/native';
import { useQueryClient } from '@tanstack/react-query';
import { createContext, useContext } from 'react';
import { z } from 'zod';
import {
	currentLibraryCache,
	insertLibrary,
	onboardingStore,
	resetOnboardingStore,
	telemetryState,
	useBridgeMutation,
	useCachedLibraries,
	useMultiZodForm,
	useOnboardingStore,
	usePlausibleEvent
} from '@sd/client';
import { toast } from '~/components/primitive/Toast';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { currentLibraryStore } from '~/utils/nav';

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

export const shareTelemetrySchema = z.union([
	z.literal('full'),
	z.literal('minimal'),
	z.literal('none')
]);

const schemas = {
	NewLibrary: z.object({
		name: z.string().min(1, 'Name is required').regex(/[\S]/g).trim()
	}),
	Privacy: z.object({
		shareTelemetry: shareTelemetrySchema
	})
};

const useFormState = () => {
	const obStore = useOnboardingStore();

	const { handleSubmit, ...forms } = useMultiZodForm({
		schemas,
		defaultValues: {
			NewLibrary: obStore.data?.['new-library'] ?? undefined,
			Privacy: obStore.data?.privacy ?? {
				shareTelemetry: 'full'
			}
		},
		onData: (data) => (onboardingStore.data = data)
	});

	const navigation = useNavigation<OnboardingStackScreenProps<any>['navigation']>();
	const submitPlausibleEvent = usePlausibleEvent();

	const queryClient = useQueryClient();
	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (lib) => {
			// We do this instead of invalidating the query because it triggers a full app re-render??
			insertLibrary(queryClient, lib);
		}
	});

	const submit = handleSubmit(
		async (data) => {
			navigation.navigate('CreatingLibrary');

			// opted to place this here as users could change their mind before library creation/onboarding finalization
			// it feels more fitting to configure it here (once)
			telemetryState.telemetryLevelPreference = data.Privacy.shareTelemetry;

			try {
				// show creation screen for a bit for smoothness
				const [library] = await Promise.all([
					createLibrary.mutateAsync({
						name: data.NewLibrary.name,
						default_locations: null
					}),
					new Promise((res) => setTimeout(res, 500))
				]);

				if (telemetryState.telemetryLevelPreference === 'full') {
					submitPlausibleEvent({ event: { type: 'libraryCreate' } });
				}

				resetOnboardingStore();

				// Switch to the new library
				currentLibraryStore.id = library.uuid;
			} catch (e) {
				toast.error('Failed to create library');
				resetOnboardingStore();
				navigation.navigate('GetStarted');
			}
		},
		(key) => navigation.navigate(key)
	);

	return { submit, forms };
};

export const useOnboardingContext = () => {
	const ctx = useContext(OnboardingContext);

	if (!ctx)
		throw new Error('useOnboardingContext must be used within OnboardingContext.Provider');

	return ctx;
};
