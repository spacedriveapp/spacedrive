import { useCallback } from 'react';
import { Particles } from 'react-tsparticles';
import { loadFull } from 'tsparticles';

const options: NonNullable<React.ComponentProps<typeof Particles>['options']> = {
	fpsLimit: 120,
	interactivity: {
		events: {
			onClick: {
				enable: false,
				mode: 'push'
			},
			resize: false
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
				enable: true,
				area: 900
			},
			value: 100
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
	const particlesInit = useCallback<NonNullable<React.ComponentProps<typeof Particles>['init']>>(
		async (engine) => {
			await loadFull(engine);
		},
		[]
	);

	return (
		<Particles id="tsparticles" className="absolute z-0" init={particlesInit} options={options} />
	);
};
