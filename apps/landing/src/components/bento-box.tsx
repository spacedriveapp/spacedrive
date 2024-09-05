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
				'flex flex-col',
				'shrink-0 flex-col justify-between rounded-[10px]',
				'bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] px-[29px] pb-[30px]'
			)}
		>
			<div className="flex h-full place-items-center justify-center px-5 pb-2 pt-5 max-xl:justify-start">
				<Image
					loading="eager"
					className="fade-in"
					width={imageWidth}
					height={imageHeight}
					alt={imageAlt}
					src={imageSrc}
				/>
			</div>
			<hgroup className="ms-3 flex max-w-screen-sm flex-col gap-2 px-2">
				<div className="mb-1.5 inline-flex items-center">
					<span
						aria-hidden
						className="-ml-3.5 mr-2.5 h-5 w-1 rounded-full"
						style={{
							backgroundColor: `${titleColor}`
						}}
					/>
					<h3 className="text-[20px] font-semibold leading-[100%] tracking-[-0.4px]">
						{title}
					</h3>
				</div>
				<p className="text-left text-base leading-snug tracking-wide text-ink-faint">
					{description}
				</p>
			</hgroup>
		</div>
	);
}
