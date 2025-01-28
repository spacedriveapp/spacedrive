/* eslint-disable tailwindcss/no-contradicting-classname */
import { Discord } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import Link from 'next/link';
import { ComponentProps, ReactNode } from 'react';
import { ExternalLinkRegex } from '~/utils/regex-external-link';

const DISCORD_HREF = 'https://discord.gg/gTaF2Z44f5';

interface CtaSecondaryButtonProps extends ComponentProps<'button'> {
	icon?: ReactNode;
	children?: ReactNode;
	href?: string;
	target?: string;
}

export function CtaSecondaryButton({
	icon = <Discord fill="#CBDBEC" className="size-5 opacity-60" />,
	children = 'Chat on Discord',
	href = DISCORD_HREF,
	target = href.match(ExternalLinkRegex)?.length ? '_blank' : undefined,
	disabled,
	...props
}: CtaSecondaryButtonProps & { disabled?: boolean }) {
	const sharedClasses = clsx(
		props.className,
		'noise with-rounded-2px-border-images inline-flex min-w-52 flex-row items-center justify-center gap-x-2.5 overflow-hidden rounded-xl px-3 py-2 transition-all',
		'bg-[#213448]/40 [--border-image:linear-gradient(to_bottom,hsl(210_22%_37%/40%),hsl(220_7%_68%/0%)75%)]',
		!disabled && 'cursor-pointer hover:brightness-110'
	);

	if (disabled) {
		return (
			<div className={sharedClasses}>
				{icon}
				<span className="text-center font-sans text-[16px] font-[600] leading-normal text-white opacity-80">
					{children}
				</span>
			</div>
		);
	}

	return (
		<Link href={href} target={target} className={clsx(sharedClasses, 'no-underline')}>
			{icon}
			<span className="text-center font-sans text-[16px] font-[600] leading-normal text-white opacity-80">
				{children}
			</span>
		</Link>
	);
}
