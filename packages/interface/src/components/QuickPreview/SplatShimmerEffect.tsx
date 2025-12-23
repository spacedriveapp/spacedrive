import { useRef, useEffect } from "react";
import * as THREE from "three";

interface SplatShimmerEffectProps {
	children: React.ReactNode;
	maskImage?: string; // Optional image URL to use as mask
}

export function SplatShimmerEffect({
	children,
	maskImage,
}: SplatShimmerEffectProps) {
	const canvasRef = useRef<HTMLDivElement>(null);
	const rafRef = useRef<number | null>(null);

	useEffect(() => {
		if (!canvasRef.current) return;

		const container = canvasRef.current;
		const width = container.clientWidth;
		const height = container.clientHeight;

		const scene = new THREE.Scene();
		const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

		const renderer = new THREE.WebGLRenderer({
			antialias: false,
			alpha: true,
			powerPreference: "high-performance",
			precision: "lowp",
			stencil: false,
			depth: false,
		});
		renderer.setSize(width, height);
		renderer.setPixelRatio(0.25); // Ultra low resolution - 16x fewer pixels
		container.appendChild(renderer.domElement);

		// Ultra simple shader
		const material = new THREE.ShaderMaterial({
			vertexShader: `
				varying vec2 vUv;
				void main() {
					vUv = uv;
					gl_Position = vec4(position, 1.0);
				}
			`,
			fragmentShader: `
				uniform float uTime;
				varying vec2 vUv;
				void main() {
					float scan = 1.0 - fract(uTime * 1.5); // Scan from top to bottom
					float dist = abs(vUv.y - scan);
					float intensity = max(0.0, 0.3 - dist);
					gl_FragColor = vec4(0.4, 0.65, 0.95, intensity);
				}
			`,
			uniforms: { uTime: { value: 0 } },
			transparent: true,
			depthWrite: false,
			depthTest: false,
		});

		const mesh = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), material);
		scene.add(mesh);

		// Throttled animation - only update every 3rd frame
		let frameCount = 0;
		const animate = () => {
			frameCount++;
			if (frameCount % 3 === 0) {
				material.uniforms.uTime.value += 0.05;
				renderer.render(scene, camera);
			}
			rafRef.current = requestAnimationFrame(animate);
		};
		rafRef.current = requestAnimationFrame(animate);

		return () => {
			if (rafRef.current) cancelAnimationFrame(rafRef.current);
			renderer.dispose();
			material.dispose();
			mesh.geometry.dispose();
			if (container.contains(renderer.domElement)) {
				container.removeChild(renderer.domElement);
			}
		};
	}, []);

	return (
		<div className="relative w-full h-full">
			{children}
			<div
				ref={canvasRef}
				className="absolute inset-0 pointer-events-none"
				style={
					maskImage
						? {
								maskImage: `url(${maskImage})`,
								maskSize: "contain",
								maskPosition: "center",
								maskRepeat: "no-repeat",
								WebkitMaskImage: `url(${maskImage})`,
								WebkitMaskSize: "contain",
								WebkitMaskPosition: "center",
								WebkitMaskRepeat: "no-repeat",
							}
						: undefined
				}
			/>
		</div>
	);
}
