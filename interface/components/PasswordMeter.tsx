import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { type getPasswordStrength } from '@sd/client';

export interface PasswordMeterProps {
	password: string;
}

export const PasswordMeter = (props: PasswordMeterProps) => {
	const [getStrength, setGetStrength] = useState<typeof getPasswordStrength | undefined>();
	const { score, scoreText } = getStrength
		? getStrength(props.password)
		: { score: 0, scoreText: 'Loading...' };

	useEffect(() => {
		let cancelled = false;

		import('@sd/client').then(({ getPasswordStrength }) => {
			if (cancelled) return;
			setGetStrength(() => getPasswordStrength);
		});

		return () => {
			cancelled = true;
		};
	}, []);

	return (
		<div className="relative">
			<h3 className="text-sm">Password strength</h3>
			<span
				className={clsx(
					'absolute right-0 top-0.5 px-1 text-sm font-semibold',
					score === 0 && 'text-red-500',
					score === 1 && 'text-red-500',
					score === 2 && 'text-amber-400',
					score === 3 && 'text-lime-500',
					score === 4 && 'text-accent'
				)}
			>
				{scoreText}
			</span>
			<div className="flex grow">
				<div className="mt-2 w-full rounded-full bg-app-box/50">
					<div
						style={{
							width: `${score !== 0 ? score * 25 : 12.5}%`
						}}
						className={clsx(
							'h-2 rounded-full transition-all',
							score === 0 && 'bg-red-500',
							score === 1 && 'bg-red-500',
							score === 2 && 'bg-amber-400',
							score === 3 && 'bg-lime-500',
							score === 4 && 'bg-accent'
						)}
					/>
				</div>
			</div>
		</div>
	);
};
