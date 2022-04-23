import { ReactComponent as AppLogo } from '../assets/app-logo.svg';
import {
  Twitter,
  Discord,
  Instagram,
  Github,
  Opencollective
} from '@icons-pack/react-simple-icons';

function FooterLink(props: { children: string; link: string }) {
  return (
    <a href={props.link} className="text-gray-300 hover:text-white">
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
            <Twitter />
            <Discord />
            <Instagram />
            <Github />
            <Opencollective />
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
        <div className="flex flex-col col-span-1 space-y-2">
          <h3 className="mb-1 text-xs font-bold uppercase ">Downloads</h3>
          <FooterLink link="#">macOS</FooterLink>
          <FooterLink link="#">Windows</FooterLink>
          <FooterLink link="#">Linux</FooterLink>
          <FooterLink link="#">Blog</FooterLink>
        </div>
        <div className="flex flex-col col-span-1 space-y-2">
          <h3 className="mb-1 text-xs font-bold uppercase ">Developers</h3>
          <FooterLink link="#">Documentation</FooterLink>
          <FooterLink link="#">Contribute</FooterLink>
          <FooterLink link="#">Extensions</FooterLink>
          <FooterLink link="#">Self Host</FooterLink>
          <FooterLink link="#">Blog</FooterLink>
        </div>
        <div className="flex flex-col col-span-1 space-y-2">
          <h3 className="mb-1 text-xs font-bold uppercase ">Company</h3>
          <FooterLink link="#">Gaming</FooterLink>
        </div>
      </div>
    </footer>
  );
}
