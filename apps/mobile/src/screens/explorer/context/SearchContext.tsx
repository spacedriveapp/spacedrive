import { create } from "zustand";

export type SearchScope = "folder" | "library";

export interface SearchFilters {
	fileTypes?: string[];
	contentTypes?: string[];
	sizeMin?: number;
	sizeMax?: number;
	dateModifiedStart?: Date;
	dateModifiedEnd?: Date;
	tags?: string[];
}

interface SearchStore {
	// Search mode state
	isSearchMode: boolean;
	query: string;
	scope: SearchScope;
	filters: SearchFilters;

	// Actions
	enterSearchMode: (query: string, scope?: SearchScope) => void;
	exitSearchMode: () => void;
	setSearchQuery: (query: string) => void;
	setSearchScope: (scope: SearchScope) => void;
	setSearchFilters: (filters: SearchFilters) => void;
}

export const useSearchStore = create<SearchStore>((set) => ({
	// Initial state
	isSearchMode: false,
	query: "",
	scope: "library",
	filters: {},

	// Actions
	enterSearchMode: (query, scope = "library") =>
		set({
			isSearchMode: true,
			query,
			scope,
		}),

	exitSearchMode: () =>
		set({
			isSearchMode: false,
			query: "",
			filters: {},
		}),

	setSearchQuery: (query) =>
		set((state) => ({
			query,
			// Auto-exit if query is cleared
			isSearchMode: query.length > 0 ? state.isSearchMode : false,
		})),

	setSearchScope: (scope) => set({ scope }),

	setSearchFilters: (filters) => set({ filters }),
}));
