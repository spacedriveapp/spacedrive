import { useNormalizedQuery } from '@sd/ts-client';

export function useSpaces() {
	return useNormalizedQuery({
		wireMethod: 'query:spaces.list',
		input: null, // Unit struct serializes as null, not {}
		resourceType: 'space',
	});
}

export function useSpaceLayout(spaceId: string | null) {
	return useNormalizedQuery({
		wireMethod: 'query:spaces.get_layout',
		input: spaceId ? { space_id: spaceId } : null,
		resourceType: 'space_layout',
		resourceId: spaceId || undefined,
		enabled: !!spaceId,
	});
}
