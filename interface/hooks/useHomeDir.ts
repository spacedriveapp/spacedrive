import { useQuery } from '@tanstack/react-query';
import { usePlatform } from '~/util/Platform';

export function useHomeDir() {
	const platform = usePlatform();

	return useQuery(['userDirs', 'home'], () => {
		if (platform.userHomeDir) return platform.userHomeDir();
		else return null;
	});
}
