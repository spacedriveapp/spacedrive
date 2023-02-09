import { create } from 'twrnc';

const tw = create(require(`../../tailwind.config.js`));

export default tw;

export const twStyle = tw.style;
