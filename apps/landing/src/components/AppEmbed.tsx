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

  return (
    <div className="w-screen ">
      <div className="relative z-30 h-[300px] lg:h-[628px] mt-16 overflow-hidden ">
        {imgFallback && <div className="h-full fade-in-image landing-img" />}
        {showApp && (
          <iframe
            referrerPolicy="origin-when-cross-origin"
            className={clsx(
              'opacity-0 pointer-events-none absolute w-[1200px] h-[300px] lg:h-[628px] z-30 border rounded-lg shadow-2xl inset-center bg-gray-850 border-gray-550',
              iFrameAppReady && 'fade-in-image !opacity-100  !pointer-events-auto'
            )}
            src={`${
              import.meta.env.VITE_SDWEB_BASE_URL || 'http://localhost:8002'
            }?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6`}
          />
        )}
      </div>
    </div>
  );
}
