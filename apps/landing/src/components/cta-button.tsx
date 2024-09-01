import { ArrowCircleDown } from '@phosphor-icons/react/dist/ssr';
import Link from 'next/link';
import { ReactNode } from 'react';
import { getCurrentPlatform, Platform } from '~/utils/current-platform';

type CtaButtonProps = { icon?: ReactNode } & (
	| {
			href: string;
			children: ReactNode;
	  }
	| {
			platform: Platform | null;
	  }
);

export function CtaButton({
	icon = <ArrowCircleDown weight="bold" size={20} />,
	...props
}: CtaButtonProps) {
	const href =
		'href' in props
			? props.href
			: `https://spacedrive.com/api/releases/desktop/stable/${props.platform?.os ?? 'linux'}/x86_64`;
	const children =
		'children' in props ? props.children : `Download for ${props.platform?.name ?? 'Linux'}`;

	return (
		<Link
			href={href}
			className="z-30 inline-flex cursor-pointer items-center justify-center gap-x-[8px] rounded-[12px] border-[1.5px] border-[#88D7FF] px-[16px] py-[10px] shadow-[0px_4px_5px_0px_rgba(168,213,255,0.25),0px_0px_39.7px_0px_rgba(75,173,255,0.50)]"
			style={{
				backgroundImage: `url('images/misc/NoisePattern.png'), linear-gradient(180deg, #42B2FD -22.29%, #0078F0 99.3%)`,
				backgroundColor: 'lightgray',
				backgroundPosition: '0% 0%',
				backgroundSize: '50px 50px',
				backgroundRepeat: 'repeat',
				backgroundBlendMode: 'overlay, normal'
			}}
		>
			{icon}
			<span className="text-center text-[16px] font-[600] leading-normal text-white">
				{children}
			</span>
		</Link>
	);
}
