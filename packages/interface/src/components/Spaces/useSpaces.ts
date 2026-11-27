import type {Space} from '@sd/ts-client';
import {useLibraryQuery} from '../../contexts/SpacedriveContext';

/** Every space in the current library. */
export function useSpaces() {
	return useLibraryQuery({type: 'spaces.list', input: {source_id: null}});
}

/** The space rooted exactly at `path` on this device, if there is one. */
export function useSpaceAt(path: string | null | undefined): Space | undefined {
	const {data} = useSpaces();
	if (!path) return undefined;
	return data?.spaces.find((space) => space.path === path);
}
