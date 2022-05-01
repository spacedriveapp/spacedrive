import Markdown from '../components/Markdown';
import React from 'react';
import { ReactComponent as Content } from '../../../../docs/product/credits.md';
import { Helmet } from 'react-helmet';

function Page() {
  return (
    <Markdown>
      <Helmet>
        <title>Our Team - Spacedrive</title>
        <meta name="description" content="Who's behind Spacedrive?" />
      </Helmet>
      <div className="team-page">
        <Content />
      </div>
    </Markdown>
  );
}

export default Page;
