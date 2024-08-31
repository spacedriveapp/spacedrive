'use client';

import { useEffect, useRef } from 'react';

export const DisclaimerBanner = () => {
	const disclaimerBannerRef = useRef<HTMLDivElement>(null);

	return (
		<div
			ref={disclaimerBannerRef}
			className="fixed bottom-0 left-0 z-[100] w-screen cursor-pointer select-none bg-black/75 px-2 py-1 text-center italic text-red-300/50"
			title="Click to hide"
			onClick={() => {
				disclaimerBannerRef.current &&
					(disclaimerBannerRef.current.style.visibility = 'hidden');
			}}
		>
			The content of this site is not final and should not be considered official marketing or
			advertising from Spacedrive Technology Inc.
		</div>
	);
};
