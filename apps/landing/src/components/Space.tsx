'use client';

import { PointMaterial, Points, Trail } from '@react-three/drei';
import { Canvas, useFrame } from '@react-three/fiber';
import { inSphere as randomInSphere } from 'maath/random';
import { useRef, useState } from 'react';
import { Color, WebGL1Renderer, WebGLRenderer, type Mesh } from 'three';

const Stars = (props: any) => {
	const ref = useRef<Mesh>();
	const [sphere] = useState(() => randomInSphere(new Float32Array(35000), { radius: 1 }));

	useFrame((_, delta) => {
		if (ref.current) {
			ref.current.rotation.x -= delta / 300;
			ref.current.rotation.y -= delta / 300;
		}
	});

	return (
		<group rotation={[0, 0, Math.PI / 4]}>
			<Points ref={ref} positions={sphere} stride={3} frustumCulled={false} {...props}>
				<PointMaterial
					transparent
					color="#ffffff"
					size={0.001}
					sizeAttenuation={true}
					depthWrite={false}
				/>
			</Points>
		</group>
	);
};

function ShootingStar() {
	const ref = useRef<any>();

	useFrame((state) => {
		const t = state.clock.getElapsedTime() * 0.5;
		if (ref.current) {
			ref.current.position.set(
				Math.sin(t) * 4,
				Math.atan(t) * Math.cos(t / 2) * 2,
				Math.cos(t) * 4
			);
		}
	});

	return (
		<Trail width={0.05} length={8} color={new Color(2, 1, 10)} attenuation={(t) => t * t}>
			<mesh ref={ref}>
				<sphereGeometry args={[0.005]} />
				<meshBasicMaterial color={[10, 1, 10]} toneMapped={false} />
			</mesh>
		</Trail>
	);
}

export default function Space({ onRenderFail }: { onRenderFail?: (error: unknown) => void }) {
	return (
		<div className="absolute left-0 top-0 z-0 h-screen w-screen bg-black opacity-50">
			<Canvas
				gl={(canvas) => {
					try {
						// https://github.com/pmndrs/react-three-fiber/blob/f2b430c/packages/fiber/src/core/index.tsx#L113-L119
						return new WebGLRenderer({
							alpha: true,
							canvas: canvas,
							antialias: true,
							powerPreference: 'high-performance'
						});
					} catch (error) {
						try {
							// Let's try WebGL1, maybe the user's browser doesn't support WebGL2
							return new WebGL1Renderer({
								alpha: true,
								canvas: canvas,
								antialias: true,
								powerPreference: 'high-performance'
							});
						} catch (error) {
							onRenderFail?.(error);
							// Return an empty renderer, just so the app doesn't crash
							return { render() {}, setSize() {}, setPixelRatio() {} };
						}
					}
				}}
				camera={{ position: [0, 0, 0] }}
			>
				<ShootingStar />
				<ShootingStar />
				<Stars />
			</Canvas>
		</div>
	);
}
