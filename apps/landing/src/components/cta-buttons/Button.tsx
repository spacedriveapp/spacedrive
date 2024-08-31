import { ArrowCircleDown } from '@phosphor-icons/react/dist/ssr';
import Link from 'next/link';

export function DownloadButton({ name, link }: { name: string; link: string }) {
	return (
		<Link
			href={link}
			className="z-30 inline-flex cursor-pointer items-center justify-center gap-[8px] rounded-[12px] border-[1.5px] border-[#88D7FF] px-[16px] py-[10px] shadow-[0px_4px_5px_0px_rgba(168,213,255,0.25),0px_0px_39.7px_0px_rgba(75,173,255,0.50)]"
			style={{
				backgroundImage: `url('images/misc/NoisePattern.png'), linear-gradient(180deg, #42B2FD -22.29%, #0078F0 99.3%)`,
				backgroundColor: 'lightgray',
				backgroundPosition: '0% 0%',
				backgroundSize: '50px 50px',
				backgroundRepeat: 'repeat',
				backgroundBlendMode: 'overlay, normal'
			}}
		>
			<ArrowCircleDown />
			Download for {name}
		</Link>
	);
}
