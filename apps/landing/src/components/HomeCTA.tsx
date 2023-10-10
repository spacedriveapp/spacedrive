import clsx from 'clsx';
import { IconProps } from '@phosphor-icons/react';
import { Button } from '@sd/ui';

interface Props {
	className?: string;
	text: string;
	icon?: IconProps;
}

export function HomeCTA({ className, text, icon }: Props) {
	return (
		<>
			<div
				className={clsx(
					'animation-delay-2 z-30 flex h-10 flex-row items-center space-x-4 fade-in',
					className
				)}
			>
				<Button
					size="lg"
					className="home-button-border-gradient relative z-30 flex cursor-pointer items-center gap-2 !rounded-[7px]
					border-0 !bg-[#2F3152]/30 py-2 text-sm !backdrop-blur-lg hover:brightness-110 md:text-[16px]"
				>
					<>
						{icon && icon}
						{text}
					</>
				</Button>
			</div>
		</>
	);
}

export default HomeCTA;
