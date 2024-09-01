import { Discord } from '@sd/assets/svgs/brands';
import Link from 'next/link';

export function DiscordButton() {
	return (
		<Link
			href="https://discord.gg/gTaF2Z44f5"
			className="z-30 inline-flex cursor-pointer items-center justify-center rounded-[12px] bg-gradient-to-b from-[#5B606A] to-[#202830] p-[1.5px]"
		>
			<span
				className="flex w-full items-center justify-center gap-x-[8px] rounded-[10.5px] bg-[rgba(33,52,72,0.46)] px-[16px] py-[10px]"
				style={{
					backgroundColor: 'rgba(33, 52, 72, 0.46)',
					backgroundImage: `url('images/misc/NoisePattern.png')`,
					backgroundPosition: '0% 0%',
					backgroundSize: '50px 50px',
					backgroundRepeat: 'repeat',
					backgroundBlendMode: 'overlay, normal'
				}}
			>
				<Discord fill="white" className="opacity-60" />
				<p className="text-center text-[16px] font-[600] leading-normal text-white opacity-80">
					Chat on Discord
				</p>
			</span>
		</Link>
	);
}
