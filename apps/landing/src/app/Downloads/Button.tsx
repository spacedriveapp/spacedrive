import { ArrowCircleDown } from '@phosphor-icons/react/dist/ssr';
import Link from 'next/link';

export function DownloadButton({ name, link }: { name: string; link: string }) {
	return (
		<Link
			href={link}
			className="flex flex-shrink-0 items-center justify-center gap-[8px] rounded-[12px] border-[1.5px] border-[#88D7FF] bg-[linear-gradient(180deg,#42B2FD_-22.29%,#0078F0_99.3%)] px-[10px] py-[16px] shadow-[0px_4px_5px_0px_rgba(168,213,255,0.25),0px_0px_39.7px_0px_rgba(75,173,255,0.50)]"
		>
			<ArrowCircleDown />
			Download for {name}
		</Link>
	);
}
