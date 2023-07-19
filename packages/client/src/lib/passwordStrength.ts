import { useEffect, useState } from 'react';

const ratings = ['Poor', 'Weak', 'Good', 'Strong', 'Perfect'];

const zxcvbnLazy = (async () => {
	const zxcvbnCommonPackage = await import('@zxcvbn-ts/language-common');
	const zxcvbnEnPackage = await import('@zxcvbn-ts/language-en');
	const { zxcvbn, zxcvbnOptions } = await import('@zxcvbn-ts/core');

	return [
		zxcvbn,
		zxcvbnOptions,
		{
			dictionary: {
				...zxcvbnCommonPackage.dictionary,
				...zxcvbnEnPackage.dictionary
			},
			graps: zxcvbnCommonPackage.adjacencyGraphs,
			translations: zxcvbnEnPackage.translations
		}
	] as const;
})();

export type StrengthResult = { scoreText: string; score: number };

export async function getPasswordStrength(password: string): Promise<StrengthResult> {
	const [zxcvbn, zxcvbnOptions, options] = await zxcvbnLazy;

	zxcvbnOptions.setOptions(options);
	const result = zxcvbn(password);
	return { scoreText: ratings[result.score]!, score: result.score };
}

// We don't use React Query so the password isn't kept in memory outside the React lifecycle.
export function usePasswordStrength(password: string): StrengthResult | null {
	const [result, setResult] = useState<StrengthResult | null>(null);

	useEffect(() => {
		getPasswordStrength(password).then(setResult);
	}, [password]);

	return result;
}
