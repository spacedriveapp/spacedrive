import clsx from 'clsx';
import React, { useRef, useState } from 'react';
import { useEffect } from 'react';
import { isMobile } from 'react-device-detect';

export default function AppEmbed() {
  const [showApp, setShowApp] = useState(false);
  const [iFrameAppReady, setIframeAppReady] = useState(false);
  const [forceImg, setForceImg] = useState(false);
  const [imgFallback, setImageFallback] = useState(false);
  const iFrame = useRef<HTMLIFrameElement>(null);

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
    }, 1000);
  }, []);

  const renderImage = (imgFallback && !iFrameAppReady) || forceImg;

  return (
    <div className="w-screen">
      <div className="relative z-30 h-[228px] px-5 sm:h-[428px] md:h-[428px] lg:h-[628px] mt-8 sm:mt-16">
        <div
          className={clsx(
            'relative h-full m-auto border rounded-lg max-w-7xl bg-gray-850 border-gray-550',
            renderImage && 'bg-transparent border-none'
          )}
        >
          {showApp && !forceImg && (
            <iframe
              ref={iFrame}
              referrerPolicy="origin-when-cross-origin"
              className={clsx(
                'w-full h-full z-30  rounded-lg shadow-iframe inset-center bg-gray-850',
                iFrameAppReady ? 'fade-in-app-embed opacity-100' : 'opacity-0 -ml-[10000px]'
              )}
              src={`${
                import.meta.env.VITE_SDWEB_BASE_URL || 'http://localhost:8002'
              }?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6`}
            />
          )}

          {renderImage && <div className="z-40 h-full fade-in-app-embed landing-img " />}
        </div>
      </div>
    </div>
  );
}
