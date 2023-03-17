import { zxcvbn, zxcvbnOptions } from '@zxcvbn-ts/core';
import zxcvbnCommonPackage from '@zxcvbn-ts/language-common';
import zxcvbnEnPackage from '@zxcvbn-ts/language-en';

const options = {
	dictionary: {
		...zxcvbnCommonPackage.dictionary,
		...zxcvbnEnPackage.dictionary
	},
	graps: zxcvbnCommonPackage.adjacencyGraphs,
	translations: zxcvbnEnPackage.translations
};

const ratings = ['Poor', 'Weak', 'Good', 'Strong', 'Perfect'];

export function getPasswordStrength(password: string): { scoreText: string; score: number } {
	zxcvbnOptions.setOptions(options);
	const result = zxcvbn(password);
	return { scoreText: ratings[result.score]!, score: result.score };
}
