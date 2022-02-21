import 'nextra-theme-docs/style.css';
import './style.css';
import Prism from 'prism-react-renderer/prism';

(typeof global !== 'undefined' ? global : window).Prism = Prism;

require('prismjs/components/prism-typescript');
require('prismjs/components/prism-rust');

export default function Nextra({ Component, pageProps }) {
  return <Component {...pageProps} />;
}
