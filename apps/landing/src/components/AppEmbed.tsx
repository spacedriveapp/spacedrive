/* eslint-disable react-hooks/exhaustive-deps */
import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';
import { getWindow } from '../utils';

const AppEmbed = () => {
	const [showApp, setShowApp] = useState(false);
	const [iFrameAppReady, setIframeAppReady] = useState(false);
	const [forceImg, setForceImg] = useState(false);
	const [imgFallback, setImageFallback] = useState(false);
	const iFrame = useRef<HTMLIFrameElement>(null);
	const window = getWindow()!;

	function handleResize() {
		if (window.innerWidth < 1000) {
			setForceImg(true);
		} else if (forceImg) {
			setForceImg(false);
		}
	}

	useEffect(() => {
		window.addEventListener('resize', handleResize);
		handleResize();
		return () => window.removeEventListener('resize', handleResize);
	}, []);

	function handleEvent(e: any) {
		if (e.data === 'spacedrive-hello') {
			if (!iFrameAppReady) setIframeAppReady(true);
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
		}, 1500);
	}, []);

	const renderImage = (imgFallback && !iFrameAppReady) || forceImg;

	const renderBloom = renderImage || iFrameAppReady;

	return (
		<div className="w-screen">
			{renderBloom && (
				<div className="relative mx-auto max-w-full sm:w-full sm:max-w-[1400px]">
					<div className="bloom burst bloom-one" />
					<div className="bloom burst bloom-three" />
					<div className="bloom burst bloom-two" />
				</div>
			)}
			<div className="relative z-30 mt-8 h-[255px] px-1 sm:mt-16 sm:h-[428px] md:h-[428px] lg:h-[628px]">
				<div
					className={clsx(
						'border-gray-550 relative m-auto h-full max-w-7xl rounded-lg border opacity-0 transition-opacity',
						renderBloom && '!opacity-100',
						renderImage && 'border-none bg-transparent'
					)}
				>
					{showApp && !forceImg && (
						<iframe
							ref={iFrame}
							referrerPolicy="origin-when-cross-origin"
							className={clsx(
								'shadow-iframe inset-center bg-gray-850  z-30 h-full w-full rounded-lg',
								iFrameAppReady ? 'fade-in-app-embed opacity-100' : '-ml-[10000px] opacity-0'
							)}
							src={`${
								import.meta.env.VITE_SDWEB_BASE_URL || 'http://localhost:8002'
							}?showControls&library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6`}
						/>
					)}

					{renderImage && <div className="fade-in-app-embed landing-img z-40 h-full w-auto" />}
				</div>
			</div>
		</div>
	);
};

export const AppEmbedPlaceholder = () => {
	return (
		<div className="relative z-30 mt-8 h-[228px] w-screen px-5 sm:mt-16 sm:h-[428px] md:h-[428px] lg:h-[628px]" />
	);
};

export default AppEmbed;
