import { ReactComponent as AppLogo } from '../assets/app-logo.svg';
import {
  Twitter,
  Discord,
  Instagram,
  Github,
  Opencollective,
  Twitch
} from '@icons-pack/react-simple-icons';

function FooterLink(props: { children: string; link: string }) {
  return (
    <a href={props.link} target="_blank" className="text-gray-300 hover:text-white">
      {props.children}
    </a>
  );
}

export function Footer() {
  return (
    <footer className="z-50 w-screen bg-gray-900 border-gray-500 border-t-1">
      <div className="container grid h-64 grid-cols-6 gap-6 pt-10 m-auto text-white">
        <div className="col-span-1">
          <AppLogo className="w-10 h-10 mb-5" />

          <h3 className="mb-1 text-xl font-bold">Spacedrive</h3>
          <p className="text-sm text-gray-350">&copy; Copyright 2022 Jamie Pine</p>
          <div className="flex flex-row mt-6 space-x-3">
            <a href="https://twitter.com/spacedriveapp" target="_blank">
              <Twitter />
            </a>
            <a href="https://discord.gg/gTaF2Z44f5" target="_blank">
              <Discord />
            </a>
            <a href="https://instagram.com/spacedriveapp" target="_blank">
              <Instagram />
            </a>
            <a href="https://github.com/spacedriveapp" target="_blank">
              <Github />
            </a>
            <a href="https://opencollective.com/spacedrive" target="_blank">
              <Opencollective />
            </a>
            <a href="https://twitch.tv/jamiepinelive" target="_blank">
              <Twitch />
            </a>
          </div>
        </div>
        <div className="col-span-1"></div>
        <div className="flex flex-col col-span-1 space-y-2">
          <h3 className="mb-1 text-xs font-bold uppercase ">About</h3>

          <FooterLink link="#">Team</FooterLink>
          <FooterLink link="#">FAQ</FooterLink>
          <FooterLink link="#">Mission</FooterLink>
          <FooterLink link="#">Changelog</FooterLink>
          <FooterLink link="#">Blog</FooterLink>
        </div>
        <div className="flex flex-col col-span-1 space-y-2 opacity-50 pointer-events-none">
          <h3 className="mb-1 text-xs font-bold uppercase ">Downloads</h3>
          <FooterLink link="#">macOS</FooterLink>
          <FooterLink link="#">Windows</FooterLink>
          <FooterLink link="#">Linux</FooterLink>
        </div>
        <div className="flex flex-col col-span-1 space-y-2">
          <h3 className="mb-1 text-xs font-bold uppercase ">Developers</h3>
          <FooterLink link="#">Documentation</FooterLink>
          <FooterLink link="#">Contribute</FooterLink>
          <FooterLink link="#">Extensions</FooterLink>
          <FooterLink link="#">Self Host</FooterLink>
        </div>
        <div className="flex flex-col col-span-1 space-y-2">
          <h3 className="mb-1 text-xs font-bold uppercase ">Org</h3>
          <FooterLink link="#">Open Collective</FooterLink>
          <FooterLink link="#">Privacy</FooterLink>
          <FooterLink link="#">Terms</FooterLink>
          <FooterLink link="#">License</FooterLink>
        </div>
      </div>
    </footer>
  );
}
