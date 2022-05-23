import React from 'react';
import Particles from 'react-tsparticles';
import { loadFull } from 'tsparticles';

export const Bubbles = () => {
	const particlesInit = async (main: any) => {
		console.log(main);
		await loadFull(main);
	};

	const particlesLoaded = (container: any) => {
		console.log(container);
	};

	return (
		//@ts-ignore
		<Particles
			id="tsparticles"
			className="absolute z-0"
			init={particlesInit}
			//@ts-ignore
			loaded={particlesLoaded}
			options={{
				fpsLimit: 120,
				interactivity: {
					events: {
						onClick: {
							enable: true,
							mode: 'push'
						},
						resize: true
					}
				},
				particles: {
					color: {
						value: '#ffffff'
					},
					collisions: {
						enable: true
					},
					move: {
						direction: 'none',
						enable: true,
						outModes: {
							default: 'bounce'
						},
						random: false,
						speed: 0.2,
						straight: false
					},
					number: {
						density: {
							enable: true,
							area: 800
						},
						value: 80
					},
					opacity: {
						value: 0.2
					},
					shape: {
						type: 'circle'
					},
					size: {
						value: { min: 0.5, max: 3 }
					}
				},
				detectRetina: true
			}}
		/>
	);
};
