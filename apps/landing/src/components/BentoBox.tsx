import Image from 'next/image';

interface BentoBoxProps {
	imageSrc: string;
	imageAlt: string;
	title: string;
	titleColor: string;
	description: string;
}

export function BentoBox({ imageSrc, imageAlt, title, titleColor, description }: BentoBoxProps) {
	return (
		<div className="flex h-[440px] w-[400px] shrink-0 flex-col justify-between rounded-[10px] border border-[#16171D] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] p-[20px]">
			<div className="flex grow flex-col items-center justify-center">
				<Image
					loading="eager"
					className="fade-in"
					width={200}
					height={200}
					alt={imageAlt}
					src={imageSrc}
				/>
			</div>
			<div className="my-2">
				<div className="inline-flex items-center gap-2 pb-[10px]">
					<div className={`bg-[ h-[15px] w-[4px] rounded-[11px]${titleColor}]`} />
					<h3 className="text-bold text-[20px]">{title}</h3>
				</div>
				<div className="text-left text-ink-faint">{description}</div>
			</div>
		</div>
	);
}
