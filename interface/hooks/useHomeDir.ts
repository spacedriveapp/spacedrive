import { useSuspenseQuery } from '@tanstack/react-query';
import { usePlatform } from '~/util/Platform';

export function useHomeDir() {
	const platform = usePlatform();

	return useSuspenseQuery({
		queryKey: ['userDirs', 'home'],
		queryFn: () => {
			if (platform.userHomeDir) return platform.userHomeDir();
			else return null;
		}
	});
}
