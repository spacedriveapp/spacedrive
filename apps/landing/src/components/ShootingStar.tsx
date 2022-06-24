// import { R } from '@icons-pack/react-simple-icons';
// import React from 'react';
import React, { useEffect } from 'react';

interface starProps {
	min: number;
	max: number;
}

// TODO:  Rerender the component randomly in a different position each time.
const Star = (props: starProps) => {
	let randomX = Math.floor(Math.random() * (props.min - props.max));
	let randomY = Math.floor(Math.random() * (props.min - props.max));
	let rarity = Math.floor(Math.random() * 10000);
	return (
		<div
			className="star"
			style={{ left: randomX, top: randomY, animationDelay: `${rarity}ms` }}
		></div>
	);
};

export const ShootingStars = () => {
	return (
		<div className="fixed z-0 w-full h-full top-0 opacity-75">
			<div className="w-full h-full rotate-45">
				<Star min={850} max={400} />
				<Star min={20} max={390} />
			</div>
		</div>
	);
};
