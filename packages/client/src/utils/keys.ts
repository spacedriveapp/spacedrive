import { z } from 'zod';
import { HashingAlgorithm } from '../core';

export const hashingAlgoSlugSchema = z.union([
	z.literal("Argon2id-s"),
	z.literal("Argon2id-h"),
	z.literal("Argon2id-p"),
	z.literal("BalloonBlake3-s"),
	z.literal("BalloonBlake3-h"),
	z.literal("BalloonBlake3-p"),
])

export type HashingAlgoSlug = z.infer<typeof hashingAlgoSlugSchema>;

export const HASHING_ALGOS = {
	'Argon2id-s': { name: 'Argon2id', params: 'Standard' },
	'Argon2id-h': { name: 'Argon2id', params: 'Hardened' },
	'Argon2id-p': { name: 'Argon2id', params: 'Paranoid' },
	'BalloonBlake3-s': { name: 'BalloonBlake3', params: 'Standard' },
	'BalloonBlake3-h': { name: 'BalloonBlake3', params: 'Hardened' },
	'BalloonBlake3-p': { name: 'BalloonBlake3', params: 'Paranoid' }
} as const satisfies Record<HashingAlgoSlug, HashingAlgorithm>;

export const slugFromHashingAlgo = (hashingAlgorithm: HashingAlgorithm): HashingAlgoSlug => 
	Object.entries(HASHING_ALGOS).find(
		([_, hashAlg]) =>
			hashAlg.name === hashingAlgorithm.name && hashAlg.params === hashingAlgorithm.params
	)![0] as HashingAlgoSlug;
