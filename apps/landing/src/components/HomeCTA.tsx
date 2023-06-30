import { ReactComponent as Alert } from '@sd/assets/svgs/alert.svg';
import { Github } from '@sd/assets/svgs/brands';
import { ReactComponent as Info } from '@sd/assets/svgs/info.svg';
import { ReactComponent as Spinner } from '@sd/assets/svgs/spinner.svg';
import clsx from 'clsx';
import { useState } from 'react';
import { useForm } from 'react-hook-form';
import { Button, Input } from '@sd/ui';

export function HomeCTA() {
	return (
		<>
			<div className="animation-delay-2 z-30 flex h-10 flex-row items-center space-x-4 fade-in">
				<Button size="lg" className="z-30 cursor-pointer border-0" variant="accent">
					Download for Mac
				</Button>
			</div>
			<p
				className={clsx(
					'animation-delay-3 z-30 mt-3 px-6 text-center text-sm text-gray-400 fade-in'
				)}
			>
				Alpha v0.1.4 <span className="mx-2 opacity-50">|</span> macOS 12+
			</p>
		</>
	);
}

export default HomeCTA;
