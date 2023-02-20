import { create } from 'twrnc';

const tw = create(require(`../../tailwind.config.js`));

const twStyle = tw.style;

export { tw, twStyle };
