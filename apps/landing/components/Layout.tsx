import React, { ReactNode } from 'react';
import Link from 'next/link';
import Head from 'next/head';

type Props = {
  children?: ReactNode;
  title?: string;
};

const Layout = ({ children, title = 'Spacedrive' }: Props) => (
  <>
    <Head>
      <title>{title}</title>
      <meta charSet="utf-8" />
      <meta name="viewport" content="initial-scale=1.0, width=device-width" />
    </Head>
    <header>
      {/* <nav>
        <Link href="/">
          <a>Home</a>
        </Link>{' '}
      </nav> */}
    </header>
    <div className="flex flex-col items-center h-screen p-1 bg-[#0B0B10] text-white">
      {children as any}
    </div>
    {/* <footer className="bg-gray-100 ">
      <span>Version 0.1.0</span>
    </footer> */}
  </>
);

export default Layout;
