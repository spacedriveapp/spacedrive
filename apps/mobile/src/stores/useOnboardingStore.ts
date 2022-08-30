import create from 'zustand';

interface OnboardingState {
	showOnboarding: boolean;
	hideOnboarding: () => void;
}

export const useOnboardingStore = create<OnboardingState>((set) => ({
	showOnboarding: true,
	hideOnboarding: () => set((state) => ({ showOnboarding: false }))
}));
