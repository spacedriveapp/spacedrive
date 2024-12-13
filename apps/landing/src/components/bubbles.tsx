import Particles, { initParticlesEngine } from '@tsparticles/react';
import { useEffect, useState } from 'react';
import { loadFull } from 'tsparticles';

const options: NonNullable<React.ComponentProps<typeof Particles>['options']> = {
	fpsLimit: 120,
	interactivity: {
		events: {
			onClick: {
				enable: false,
				mode: 'push'
			},
			resize: {
				enable: false
			}
		}
	},
	particles: {
		color: {
			value: '#ffffff'
		},
		collisions: {
			enable: false
		},
		move: {
			direction: 'top',
			enable: true,
			outModes: {
				default: 'destroy'
			},
			random: false,
			speed: 0.2,
			straight: true
		},
		number: {
			density: {
				enable: true
			},
			value: 400
		},
		opacity: {
			value: 0.1
		},
		shape: {
			type: 'circle'
		},
		size: {
			value: { min: 0.5, max: 3 }
		}
	},
	detectRetina: true
};

export const Bubbles = () => {
	const [init, setInit] = useState(false);

	useEffect(() => {
		// this should be run only once per application lifetime
		initParticlesEngine(async (engine) => {
			await loadFull(engine);
		}).then(() => {
			setInit(true);
		}, console.error);
	}, []);

	if (init)
		return <Particles id="tsparticles" className="absolute inset-0 z-0" options={options} />;
};
