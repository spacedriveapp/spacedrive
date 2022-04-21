import clsx from 'clsx';
import React, { useEffect, useState } from 'react';
import { ReactComponent as AppLogo } from '../assets/app-logo.svg';

function NavLink(props: { children: string }) {
  return (
    <a className="p-4 text-gray-300 no-underline transition cursor-pointer hover:text-gray-50">
      {props.children}
    </a>
  );
}

export default function NavBar() {
  const [isAtTop, setIsAtTop] = useState(true);

  function onScroll(event: Event) {
    if (window.pageYOffset > 20) setIsAtTop(true);
    else if (isAtTop) setIsAtTop(false);
  }

  useEffect(() => {
    window.addEventListener('scroll', onScroll);
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  return (
    <div
      className={clsx(
        'fixed transition z-50 w-full h-16 border-b backdrop-blur',
        isAtTop ? 'border-gray-550 bg-gray-750 bg-opacity-80' : 'bg-transparent border-transparent'
      )}
    >
      <div className="container flex items-center h-full m-auto ">
        <AppLogo className="z-30 w-8 h-8 mr-3" />
        <h3 className="text-xl font-bold text-white">
          Memes
          <span className="ml-2 text-xs text-gray-400 uppercase">BETA</span>
        </h3>

        <div className="space-x-4 text-white mx-28">
          <NavLink>Product</NavLink>
          <NavLink>Developers</NavLink>
          <NavLink>Documentation</NavLink>
          <NavLink>Support</NavLink>
          <NavLink>Download</NavLink>
        </div>
      </div>
    </div>
  );
}
