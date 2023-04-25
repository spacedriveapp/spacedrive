import { OperatingSystem } from '~/util/Platform';

export const macOnly = (platform: OperatingSystem | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';
