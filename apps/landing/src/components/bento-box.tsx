import clsx from 'clsx';
import Image from 'next/image';
import { ComponentProps } from 'react';

interface BentoBoxProps extends ComponentProps<'div'> {
	imageSrc: string;
	imageAlt: string;
	title: string;
	titleColor: string;
	description: string;
	imageWidth?: number;
	imageHeight?: number;
}

export function BentoBox({
	imageSrc,
	imageAlt,
	title,
	titleColor,
	description,
	imageHeight = 250,
	imageWidth = 250,
	className,
	...rest
}: BentoBoxProps) {
	return (
		<div
			{...rest}
			className={clsx(
				className,
				'relative',
				'flex h-[440px] w-full max-w-[375px]',
				'shrink-0 flex-col justify-between rounded-[10px]',
				'bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] px-[29px] pb-[30px]'
			)}
		>
			<div className="flex flex-col items-center justify-center grow">
				<Image
					loading="eager"
					className="fade-in"
					width={imageWidth}
					height={imageHeight}
					alt={imageAlt}
					src={imageSrc}
				/>
			</div>
			<div className="mx-4">
				<div className="inline-flex h-[102] w-[342] items-center pb-[10px]">
					<div
						className={`mr-[10px] h-[15px] w-[4px] rounded-[11px]`}
						style={{
							backgroundColor: `${titleColor}`
						}}
					/>
					<h3 className="text-[20px] font-[700] leading-[100%] tracking-[-0.4px]">
						{title}
					</h3>
				</div>
				<div className="text-left text-[16px] font-[400] leading-[24px] tracking-[0.16px] text-ink-faint">
					{description}
				</div>
			</div>
		</div>
	);
}
