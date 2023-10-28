// import clsx from 'clsx';
// import { useEffect, useRef, useState } from 'react';
// import { getWindow } from '~/utils/util';

// const AppEmbed = () => {
// 	const [showApp, setShowApp] = useState(false);
// 	const [iFrameAppReady, setIframeAppReady] = useState(false);
// 	const [forceImg, setForceImg] = useState(false);
// 	const [imgFallback, setImageFallback] = useState(false);
// 	// const iFrame = useRef<HTMLIFrameElement>(null);
// 	const window = getWindow()!;

// 	// useEffect(() => {
// 	// 	function handleResize() {
// 	// 		if (window.innerWidth < 1000) {
// 	// 			setForceImg(true);
// 	// 		} else if (forceImg) {
// 	// 			setForceImg(false);
// 	// 		}
// 	// 	}
// 	// 	window.addEventListener('resize', handleResize);
// 	// 	handleResize();
// 	// 	return () => window.removeEventListener('resize', handleResize);
// 	// }, [forceImg, window]);

// 	// after five minutes kill the live demo
// 	// useEffect(() => {
// 	// 	const timer = setTimeout(() => {
// 	// 		setIframeAppReady(false);
// 	// 	}, 300000);
// 	// 	return () => clearTimeout(timer);
// 	// }, []);

// 	// useEffect(() => {
// 	// 	function handleEvent(e: any) {
// 	// 		if (e.data === 'spacedrive-hello') {
// 	// 			if (!iFrameAppReady) setIframeAppReady(true);
// 	// 		}
// 	// 	}
// 	// 	window.addEventListener('message', handleEvent, false);
// 	// 	setShowApp(true);

// 	// 	return () => window.removeEventListener('message', handleEvent);
// 	// }, [iFrameAppReady, window]);

// 	// useEffect(() => {
// 	// 	setTimeout(() => {
// 	// 		if (!iFrameAppReady) setImageFallback(true);
// 	// 	}, 1500);
// 	// }, [iFrameAppReady]);

// 	const renderImage = (imgFallback && !iFrameAppReady) || forceImg;

// 	const renderBloom = renderImage || iFrameAppReady;

// 	return (
// 		<div className="w-screen">
// 			{/* {renderBloom && ( */}
// 			<div className="relative mx-auto max-w-full sm:w-full sm:max-w-[1400px]">
// 				<div className="bloom burst bloom-one" />
// 				<div className="bloom burst bloom-three" />
// 				<div className="bloom burst bloom-two" />
// 			</div>
// 			{/* )} */}
// 			<div className="z-30 mt-8 flex h-[255px] w-full px-6 sm:mt-20 sm:h-[428px] md:h-[428px] lg:h-[628px]">
// 				<div className="relative m-auto flex h-full w-full max-w-7xl rounded-lg border border-black transition-opacity">
// 					<div className="z-30 flex w-full rounded-lg border-t border-app-line/50 bg-app/30 backdrop-blur">
// 						<img className="rounded-lg" src="/images/test.png" />
// 					</div>
// 				</div>
// 			</div>
// 		</div>
// 	);
// };

// export const AppEmbedPlaceholder = () => {
// 	return (
// 		<div className="relative z-30 mt-8 h-[228px] w-screen px-5 sm:mt-16 sm:h-[428px] md:h-[428px] lg:h-[628px]" />
// 	);
// };

// export default AppEmbed;
