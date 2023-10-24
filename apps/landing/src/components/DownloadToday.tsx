import { Download } from '@phosphor-icons/react/dist/ssr';
import clsx from 'clsx';
import { useInView } from 'framer-motion';
import React, { useRef } from 'react';
import { Button } from '@sd/ui';

interface Props {
	isWindows?: boolean;
}

const DownloadToday = ({ isWindows }: Props) => {
	const ref = useRef<HTMLDivElement>(null);
	const isInView = useInView(ref, {
		amount: 0.5,
		once: true
	});
	return (
		<div
			ref={ref}
			className={clsx(
				'relative mb-[150px] mt-10 flex h-[250px] w-full max-w-7xl flex-col justify-center bg-app-box/30 opacity-0',
				'overflow-hidden rounded-md p-2 text-center md:mb-[250px] md:h-[350px]',
				isInView && 'fade-in-heading'
			)}
		>
			<div className="relative z-10">
				<h1 className="mx-auto w-full max-w-[500px] text-[20px] font-semibold leading-tight md:text-[30px]">
					Ready to get organized?
				</h1>
				<Button className="mx-auto mt-5 flex gap-2" variant="accent" size="md">
					<Download size={20} />
					{isWindows ? 'Download on Windows' : 'Download on Mac'}
				</Button>
			</div>
		</div>
	);
};

export default DownloadToday;
