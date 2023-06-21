import { z } from 'zod';

export const version = z.union([z.literal('stable'), z.literal('alpha')]);
export const tauriTarget = z.union([z.literal('linux'), z.literal('windows'), z.literal('darwin')]);
export const tauriArch = z.union([z.literal('x86_64'), z.literal('aarch64')]);

export const params = z.object({
	target: tauriTarget,
	arch: tauriArch,
	version: version.or(z.string())
});

export type TauriResponse = {
	version: string;
	pub_date: string;
	url: string;
	signature: string;
	notes: string;
};
