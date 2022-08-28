/// https://github.com/ryanhefner/calculate-aspect-ratio/blob/master/src/index.js

export const gcd = (a: number, b: number): number => {
	return b ? gcd(b, a % b) : a;
};

const aspectRatio = (width: number, height: number) => {
	const divisor = gcd(width, height);

	return `${width / divisor}:${height / divisor}`;
};

export default aspectRatio;
