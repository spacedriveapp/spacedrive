export default {
  github: 'https://github.com/jamiepine/spacedrive',
  docsRepositoryBase: 'https://github.com/jamiepine/spacedrive/',
  titleSuffix: ' | Spacedrive',
  logo: (
    <>
      <span className="hidden mr-2 font-extrabold md:inline">Spacedrive</span>
      <span className="hidden font-normal text-gray-600 md:inline">
        The Virtual Private Filesystem
      </span>
    </>
  ),
  search: true,
  prevLinks: true,
  nextLinks: true,
  footer: true,
  footerEditLink: 'Edit this page on GitHub',
  footerText: <>© {new Date().getFullYear()} Jamie Pine. All rights reserved.</>,
  head: (
    <>
      <meta name="viewport" content="width=device-width, initial-scale=1.0" />
      <meta httpEquiv="Content-Language" content="en" />

      <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
      <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png" />
      <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png" />
      <link rel="manifest" href="/site.webmanifest" />
      <link rel="mask-icon" href="/safari-pinned-tab.svg" color="#000000" />
      <meta name="msapplication-TileColor" content="#ff0000" />
    </>
  )
};
