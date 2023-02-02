import { zxcvbn, zxcvbnOptions } from '@zxcvbn-ts/core';
import zxcvbnCommonPackage from '@zxcvbn-ts/language-common';
import zxcvbnEnPackage from '@zxcvbn-ts/language-en';
import clsx from 'clsx';

const options = {
	dictionary: {
		...zxcvbnCommonPackage.dictionary,
		...zxcvbnEnPackage.dictionary
	},
	graps: zxcvbnCommonPackage.adjacencyGraphs,
	translations: zxcvbnEnPackage.translations
};
zxcvbnOptions.setOptions(options);

export const PasswordMeter = (props: { password: string }) => {
	const ratings = ['Poor', 'Weak', 'Good', 'Strong', 'Perfect'];

	const zx = zxcvbn(props.password);

	const innerDiv = {
		width: `${zx.score !== 0 ? zx.score * 25 : 12.5}%`,
		height: '5px',
		borderRadius: 80
	};

	return (
		<div className="relative mt-4 mb-5 flex flex-grow">
			<div className="mt-2 h-[5px] w-4/5 rounded-[80px]">
				<div
					style={innerDiv}
					className={clsx(
						zx.score === 0 && 'bg-red-700',
						zx.score === 1 && 'bg-red-500',
						zx.score === 2 && 'bg-amber-400',
						zx.score === 3 && 'bg-lime-500',
						zx.score === 4 && 'bg-accent'
					)}
				/>
			</div>
			<span
				className={clsx(
					'absolute right-[5px] pr-1 pl-1 text-sm font-[750]',
					zx.score === 0 && 'text-red-700',
					zx.score === 1 && 'text-red-500',
					zx.score === 2 && 'text-amber-400',
					zx.score === 3 && 'text-lime-500',
					zx.score === 4 && 'text-accent'
				)}
			>
				{ratings[zx.score]}
			</span>
		</div>
	);
};
