import { Discord } from '@sd/assets/svgs/brands';
import Link from 'next/link';

export function DiscordButton() {
	return (
		<Link
			href="https://discord.gg/gTaF2Z44f5"
			className="z-30 inline-flex cursor-pointer items-center justify-center gap-[8px] rounded-[12px] px-[16px] py-[10px] text-ink-faint"
			style={{
				backgroundColor: 'rgba(33, 52, 72, 0.46)',
				backgroundImage: `url('images/misc/NoisePattern.png')`,
				backgroundPosition: '0% 0%',
				backgroundSize: '50px 50px',
				backgroundRepeat: 'repeat',
				backgroundBlendMode: 'overlay, normal',
				border: '1.5px rgba(73, 94, 115, 0.40)'
			}}
		>
			<Discord />
			Chat on Discord
		</Link>
	);
}

export default DiscordButton;
