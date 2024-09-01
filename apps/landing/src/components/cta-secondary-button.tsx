import { Discord } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import Link from 'next/link';
import { ReactNode } from 'react';

const DISCORD_HREF = 'https://discord.gg/gTaF2Z44f5';

interface CtaSecondaryButtonProps {
	icon?: ReactNode;
	children?: ReactNode;
	href?: string;
}

export function CtaSecondaryButton({
	icon = <Discord fill="#CBDBEC" className="size-5 opacity-60" />,
	children = 'Chat on Discord',
	href = DISCORD_HREF
}: CtaSecondaryButtonProps) {
	return (
		<Link
			href={href}
			className={clsx(
				'noise with-rounded-2px-border-images inline-flex min-w-52 cursor-pointer flex-row items-center justify-center gap-x-2.5 overflow-hidden rounded-xl px-3 py-2',
				'bg-[#213448]/40 [--border-image:linear-gradient(to_bottom,hsl(210_22%_37%/40%),hsl(220_7%_68%/0%)75%)]'
			)}
		>
			{icon}
			<span className="text-center text-[16px] font-[600] leading-normal text-white opacity-80">
				{children}
			</span>
		</Link>
	);
}
