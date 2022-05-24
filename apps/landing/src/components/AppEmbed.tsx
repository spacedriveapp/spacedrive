import clsx from 'clsx';
import React, { useState } from 'react';
import { useEffect } from 'react';
import { isMobile } from 'react-device-detect';

export default function AppEmbed() {
	const [showApp, setShowApp] = useState(false);
	const [iFrameAppReady, setIframeAppReady] = useState(false);
	const [imgFallback, setImageFallback] = useState(false);

	function handleEvent(e: any) {
		if (e.data === 'spacedrive-hello') {
			if (!iFrameAppReady && !isMobile) setIframeAppReady(true);
		}
	}

	// after five minutes kill the live demo
	useEffect(() => {
		const timer = setTimeout(() => {
			setIframeAppReady(false);
		}, 300000);
		return () => clearTimeout(timer);
	}, []);

	useEffect(() => {
		window.addEventListener('message', handleEvent, false);
		setShowApp(true);

		return () => window.removeEventListener('message', handleEvent);
	}, []);

	useEffect(() => {
		setTimeout(() => {
			if (!iFrameAppReady) setImageFallback(true);
		}, 1000);
	}, []);

	return isMobile ? null : (
		<div className="w-screen">
			<div className="relative z-30 h-[200px] p-2 sm:p-0 sm:h-[328px] lg:h-[628px] mt-8 sm:mt-16 overflow-hidden ">
				{showApp && (
					<iframe
						referrerPolicy="origin-when-cross-origin"
						className={clsx(
							'absolute w-[1200px] h-[300px] lg:h-[628px] z-30 border rounded-lg shadow-2xl inset-center bg-gray-850 border-gray-550',
							iFrameAppReady ? 'fade-in-image opacity-100' : 'opacity-0 -ml-[10000px]'
						)}
						src={`${
							import.meta.env.VITE_SDWEB_BASE_URL || 'http://localhost:8002'
						}?showControls&library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6`}
					/>
				)}
				{imgFallback && !iFrameAppReady && (
					<div className="z-40 h-full fade-in-image landing-img" />
				)}
			</div>
		</div>
	);
}
