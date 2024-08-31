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
		<div className="h-[440px] w-[400px] flex-shrink-0 rounded-[10px] border border-[#16171D] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] p-[29px]">
			<Image
				loading="eager"
				className="flex items-center justify-center fade-in"
				width={200}
				height={200}
				alt={imageAlt}
				src={imageSrc}
			/>
			<div className="inline-flex items-center justify-center gap-2 pb-[10px]">
				<div className={`h-[15px] w-[4px] rounded-[11px] bg-[${titleColor}]`} />
				<h3 className="text-[20px]">{title}</h3>
			</div>
			<div className="text-md inline-flex items-center justify-center gap-2 text-ink-faint">
				{description}
			</div>
		</div>
	);
}
