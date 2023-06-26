import { useState, RefObject, useCallback, useEffect } from 'react';

export const useIsTextTruncated = (element: RefObject<HTMLElement>, text: string | null): boolean => {
  const determineIsTruncated = useCallback((): boolean => {
    if (!element.current) return false;
    return element.current.scrollWidth > element.current.clientWidth;
}, [element]);


  const [isTruncated, setIsTruncated] = useState<boolean>(determineIsTruncated());

 useEffect(() => {
    const resizeListener = (): void => {
      setIsTruncated(determineIsTruncated());
    };

    window.addEventListener('resize', resizeListener);

    return () => {
      window.removeEventListener('resize', resizeListener);
    };
  }, [element, determineIsTruncated, text]);

 useEffect(() => {
    setIsTruncated(determineIsTruncated());
  }, [element, determineIsTruncated, text]);

  return isTruncated;
};
