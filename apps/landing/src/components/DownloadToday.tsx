import { Download } from 'phosphor-react';
import React from 'react';
import { Button } from '@sd/ui';

const DownloadToday = () => {
	return (
		<div
			className="download-today-border-gradient download-today-shadow-inset relative mb-[150px] flex h-[250px] w-full max-w-[1000px] flex-col justify-center
		overflow-hidden rounded-md bg-gradient-to-b from-transparent to-[#090816] p-2 text-center md:mb-[250px] md:h-[350px]"
		>
			<div className="absolute left-0 top-0 h-full w-full bg-gradient-to-t from-[#090816] to-transparent" />
			<div className="relative z-10">
				<h1 className="mx-auto w-full max-w-[500px] text-[20px] font-semibold md:text-[30px]">
					Download Spacedrive today and enjoy the experience of the future
				</h1>
				<Button
					size="lg"
					className="mx-auto mt-7 flex w-fit cursor-pointer items-center justify-center gap-2 !rounded-md
					border-0  bg-gradient-to-r from-violet-400 to-fuchsia-400 py-2 text-sm hover:brightness-110"
				>
					<Download size={20} />
					Download
				</Button>
			</div>
		</div>
	);
};

export default DownloadToday;
