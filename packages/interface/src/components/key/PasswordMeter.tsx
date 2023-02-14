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

	const widthCalcStyle = {
		width: `${zx.score !== 0 ? zx.score * 25 : 12.5}%`
	};

	return (
		<div className="relative ">
			<h3 className="text-sm">Password strength</h3>
			<span
				className={clsx(
					'absolute font-semibold top-0.5 right-0 text-sm pr-1 pl-1',
					zx.score === 0 && 'text-red-500',
					zx.score === 1 && 'text-red-500',
					zx.score === 2 && 'text-amber-400',
					zx.score === 3 && 'text-lime-500',
					zx.score === 4 && 'text-accent'
				)}
			>
				{ratings[zx.score]}
			</span>
			<div className="flex flex-grow ">
				<div className="w-full mt-2 rounded-full bg-app-box/50">
					<div
						style={widthCalcStyle}
						className={clsx(
							'h-2 rounded-full',
							zx.score === 0 && 'bg-red-500',
							zx.score === 1 && 'bg-red-500',
							zx.score === 2 && 'bg-amber-400',
							zx.score === 3 && 'bg-lime-500',
							zx.score === 4 && 'bg-accent'
						)}
					/>
				</div>
			</div>
		</div>
	);
};
