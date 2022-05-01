import Markdown from '../components/Markdown';
import React from 'react';
import { ReactComponent as Content } from '../../../../docs/product/faq.md';
import { Helmet } from 'react-helmet';

function Page() {
  return (
    <Markdown>
      <Helmet>
        <title>FAQ - Spacedrive</title>
        <meta name="description" content="Updates and release builds of the Spacedrive app." />
      </Helmet>
      <Content />
    </Markdown>
  );
}

export default Page;
