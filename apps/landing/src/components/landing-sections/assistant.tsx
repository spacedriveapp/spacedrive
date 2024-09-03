import React from 'react';

export const Assistant = () => {
	return (
		<div className="flex w-full flex-col p-4">
			<div>
				<div className="inline-flex items-center justify-center gap-[10px] rounded-full border-2 border-[#FC79E7] bg-[rgba(43,43,59,0.50)] px-[10px] py-[11px]">
					<p className="text-center text-[14px] font-[500] leading-[125%]">
						COMING NEXT YEAR
					</p>
				</div>
				<h1 className="pt-[16px] text-xl">
					Assistant. Mighty-powerful AI — no cloud needed.
				</h1>
				<h2 className="pt-[16px] text-lg text-ink-faint">Details to be revealed soon...</h2>
			</div>
		</div>
	);
};
