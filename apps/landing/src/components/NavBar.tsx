import { Button } from '@sd/ui';
import clsx from 'clsx';
import React, { useEffect, useState } from 'react';
import { List } from 'phosphor-react';
import { ReactComponent as AppLogo } from '../assets/app-logo.svg';
import { Discord, Github } from '@icons-pack/react-simple-icons';

function NavLink(props: { link?: string; children: string }) {
  return (
    <a
      href={props.link ?? '#'}
      target="_blank"
      className="p-4 text-gray-300 no-underline transition cursor-pointer hover:text-gray-50"
    >
      {props.children}
    </a>
  );
}

export default function NavBar() {
  const [isAtTop, setIsAtTop] = useState(window.pageYOffset < 20);

  function onScroll(event: Event) {
    if (window.pageYOffset < 20) setIsAtTop(true);
    else if (isAtTop) setIsAtTop(false);
  }

  useEffect(() => {
    window.addEventListener('scroll', onScroll);
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  return (
    <div
      className={clsx(
        'fixed transition z-50 w-full h-16 border-b ',
        isAtTop
          ? 'bg-transparent border-transparent'
          : 'border-gray-550 bg-gray-750 bg-opacity-80 backdrop-blur'
      )}
    >
      <div className="container relative flex items-center h-full px-5 m-auto">
        <div className="absolute flex flex-row items-center">
          <AppLogo className="z-30 w-8 h-8 mr-3" />
          <h3 className="text-xl font-bold text-white">
            Spacedrive
            <span className="ml-2 text-xs text-gray-400 uppercase">BETA</span>
          </h3>
        </div>

        <div className="hidden m-auto space-x-4 text-white lg:block ">
          <NavLink link="https://github.com/spacedriveapp/#features">Features</NavLink>
          <NavLink link="https://github.com/spacedriveapp/spacedrive/tree/main/docs">Docs</NavLink>
          <NavLink link="https://github.com/spacedriveapp/spacedrive/blob/main/docs/product/faq.md">
            FAQ
          </NavLink>
          <NavLink link="https://github.com/spacedriveapp/spacedrive/tree/main/docs/changelog">
            Changelog
          </NavLink>
          <NavLink link="https://opencollective.com/spacedrive">Sponsor us</NavLink>
        </div>
        <a href="#footer">
          <Button className="absolute top-3 block !p-1 right-3 lg:hidden">
            <List weight="bold" className="w-6 h-6" />
          </Button>
        </a>
        <div className="absolute flex-row hidden space-x-5 right-3 lg:flex">
          <a href="https://discord.gg/gTaF2Z44f5" target="_blank">
            <Discord className="text-white" />
          </a>
          <a href="https://discord.gg/gTaF2Z44f5" target="_blank">
            <Github className="text-white" />
          </a>
        </div>
      </div>
    </div>
  );
}
