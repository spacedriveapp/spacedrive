import { clsx } from 'clsx';
import { t } from 'i18next';
import React, { useEffect, useRef } from 'react';

const StarfieldEffect: React.FC<{ className?: string }> = ({ className }) => {
	const canvasRef = useRef<HTMLCanvasElement>(null);

	useEffect(() => {
		const canvas = canvasRef.current;
		if (!canvas) return;

		const ctx = canvas.getContext('2d');
		if (!ctx) return;

		const resizeCanvas = () => {
			const scale = window.devicePixelRatio || 1;
			const width = canvas.parentElement?.clientWidth || 800;
			const height = canvas.parentElement?.clientHeight || 300;
			canvas.width = width * scale;
			canvas.height = height * scale;
			canvas.style.width = `${width}px`;
			canvas.style.height = `${height}px`;
			ctx.scale(scale, scale);
		};

		resizeCanvas();
		window.addEventListener('resize', resizeCanvas);

		canvas.style.position = 'absolute';
		canvas.oncontextmenu = (e) => e.preventDefault();

		const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
		const pix = imageData.data;

		const center = { x: canvas.width / 2, y: canvas.height / 2 };

		let mouseActive = false;
		let mouseDown = false;
		let mousePos = { x: center.x, y: center.y };

		let starSpeed = 20;
		const starSpeedMin = starSpeed;
		const starSpeedMax = 100;
		const starDistance = 5000;

		let fov = 320;
		const fovMin = 210;
		const fovMax = fov;

		const starHolderCount = 8000; // Increased star count
		const starHolder: any[] = [];
		const starBgHolder: any[] = [];

		const backgroundColor = { r: 28, g: 29, b: 37, a: 0 };

		const clearImageData = () => {
			for (let i = 0, l = pix.length; i < l; i += 4) {
				pix[i] = backgroundColor.r;
				pix[i + 1] = backgroundColor.g;
				pix[i + 2] = backgroundColor.b;
				pix[i + 3] = backgroundColor.a;
			}
		};

		const setPixel = (x: number, y: number, r: number, g: number, b: number, a: number) => {
			const i = (x + y * canvas.width) * 4;
			pix[i] = r;
			pix[i + 1] = g;
			pix[i + 2] = b;
			pix[i + 3] = a;
		};

		const setPixelAdditive = (
			x: number,
			y: number,
			r: number,
			g: number,
			b: number,
			a: number
		) => {
			const i = (x + y * canvas.width) * 4;
			pix[i] = (pix[i] ?? 0) + r;
			pix[i + 1] = (pix[i + 1] ?? 0) + g;
			pix[i + 2] = (pix[i + 2] ?? 0) + b;
			pix[i + 3] = a;
		};

		const drawLine = (
			x1: number,
			y1: number,
			x2: number,
			y2: number,
			r: number,
			g: number,
			b: number,
			a: number
		) => {
			const dx = Math.abs(x2 - x1);
			const dy = Math.abs(y2 - y1);
			const sx = x1 < x2 ? 1 : -1;
			const sy = y1 < y2 ? 1 : -1;
			let err = dx - dy;
			let lx = x1;
			let ly = y1;

			const continueLoop = true;
			while (continueLoop) {
				if (lx > 0 && lx < canvas.width && ly > 0 && ly < canvas.height) {
					setPixel(lx, ly, r, g, b, a);
				}
				if (lx === x2 && ly === y2) break;
				const e2 = 2 * err;
				if (e2 > -dx) {
					err -= dy;
					lx += sx;
				}
				if (e2 < dy) {
					err += dx;
					ly += sy;
				}
			}
		};

		const addParticle = (
			x: number,
			y: number,
			z: number,
			ox: number,
			oy: number,
			oz: number
		) => {
			const particle = {
				x,
				y,
				z,
				ox,
				oy,
				x2d: 0,
				y2d: 0,
				color: { r: 0, g: 0, b: 0, a: 0 },
				oColor: { r: 0, g: 0, b: 0, a: 0 },
				w: 0,
				distance: 0,
				distanceTotal: 0
			};
			return particle;
		};

		const addParticles = () => {
			let x, y, z, colorValue, particle;
			for (let i = 0; i < starHolderCount / 3; i++) {
				x = Math.random() * 24000 - 12000;
				y = Math.random() * 4500 - 2250;
				z = Math.round(Math.random() * starDistance);
				colorValue = 185; // Adjusted color
				particle = addParticle(x, y, z, x, y, z);
				particle.color = { r: 171, g: 172, b: 185, a: 255 };
				starBgHolder.push(particle);
			}
			for (let i = 0; i < starHolderCount; i++) {
				x = Math.random() * 10000 - 5000;
				y = Math.random() * 10000 - 5000;
				z = Math.round(Math.random() * starDistance);
				colorValue = 185; // Adjusted color
				particle = addParticle(x, y, z, x, y, z);
				particle.color = { r: 171, g: 172, b: 185, a: 255 };
				particle.oColor = { r: 171, g: 172, b: 185, a: 255 };
				particle.w = 1;
				particle.distance = starDistance - z;
				particle.distanceTotal = Math.round(starDistance + fov - particle.w);
				starHolder.push(particle);
			}
		};

		const animloop = () => {
			requestAnimationFrame(animloop);
			render();
		};

		const render = () => {
			clearImageData();
			let star, scale;

			if (mouseActive) {
				starSpeed += 2;
				if (starSpeed > starSpeedMax) starSpeed = starSpeedMax;
			} else {
				starSpeed -= 1;
				if (starSpeed < starSpeedMin) starSpeed = starSpeedMin;
			}

			fov += mouseActive ? -1 : 0.5;
			fov = Math.max(fovMin, Math.min(fovMax, fov));

			const warpSpeedValue = starSpeed * (starSpeed / (starSpeedMax / 2));

			for (const bgStar of starBgHolder) {
				star = bgStar;
				scale = fov / (fov + star.z);
				star.x2d = star.x * scale + center.x;
				star.y2d = star.y * scale + center.y;
				if (
					star.x2d > 0 &&
					star.x2d < canvas.width &&
					star.y2d > 0 &&
					star.y2d < canvas.height
				) {
					setPixel(
						star.x2d | 0,
						star.y2d | 0,
						star.color.r,
						star.color.g,
						star.color.b,
						255
					);
				}
			}

			for (const mainStar of starHolder) {
				star = mainStar;
				star.z -= starSpeed;
				star.distance += starSpeed;
				if (star.z < -fov + star.w) {
					star.z = starDistance;
					star.distance = 0;
				}

				const distancePercent = star.distance / star.distanceTotal;
				star.color.r = Math.floor(star.oColor.r * distancePercent);
				star.color.g = Math.floor(star.oColor.g * distancePercent);
				star.color.b = Math.floor(star.oColor.b * distancePercent);

				scale = fov / (fov + star.z);
				star.x2d = star.x * scale + center.x;
				star.y2d = star.y * scale + center.y;

				if (
					star.x2d > 0 &&
					star.x2d < canvas.width &&
					star.y2d > 0 &&
					star.y2d < canvas.height
				) {
					setPixelAdditive(
						star.x2d | 0,
						star.y2d | 0,
						star.color.r,
						star.color.g,
						star.color.b,
						255
					);
				}

				if (starSpeed !== starSpeedMin) {
					const nz = star.z + warpSpeedValue;
					scale = fov / (fov + nz);
					const x2d = star.x * scale + center.x;
					const y2d = star.y * scale + center.y;
					if (x2d > 0 && x2d < canvas.width && y2d > 0 && y2d < canvas.height) {
						drawLine(
							star.x2d | 0,
							star.y2d | 0,
							x2d | 0,
							y2d | 0,
							star.color.r,
							star.color.g,
							star.color.b,
							255
						);
					}
				}
			}

			ctx.putImageData(imageData, 0, 0);

			center.x += (mousePos.x - center.x) * 0.015;
			if (!mouseActive) {
				center.x += (canvas.width / 2 - center.x) * 0.015;
			}
		};

		const getMousePos = (event: MouseEvent) => {
			const rect = canvas.getBoundingClientRect();
			return { x: event.clientX - rect.left, y: event.clientY - rect.top };
		};

		const mouseMoveHandler = (event: MouseEvent) => {
			mousePos = getMousePos(event);
		};

		const mouseEnterHandler = () => {
			mouseActive = true;
		};

		const mouseLeaveHandler = () => {
			mouseActive = false;
			mouseDown = false;
		};

		canvas.addEventListener('mousemove', mouseMoveHandler);
		canvas.addEventListener('mousedown', () => {
			mouseDown = true;
		});
		canvas.addEventListener('mouseup', () => {
			mouseDown = false;
		});
		canvas.addEventListener('mouseenter', mouseEnterHandler);
		canvas.addEventListener('mouseleave', mouseLeaveHandler);

		addParticles();
		animloop();

		return () => {
			canvas.removeEventListener('mousemove', mouseMoveHandler);
			canvas.removeEventListener('mousedown', () => {
				mouseDown = true;
			});
			canvas.removeEventListener('mouseup', () => {
				mouseDown = false;
			});
			canvas.removeEventListener('mouseenter', mouseEnterHandler);
			canvas.removeEventListener('mouseleave', mouseLeaveHandler);
			window.removeEventListener('resize', resizeCanvas);
		};
	}, []);

	return (
		<canvas
			ref={canvasRef}
			className={clsx(
				'block size-full rounded-lg border border-gray-500 transition-all hover:scale-105',
				className
			)}
		>
			{t('drop_files_here_to_send_with')}
		</canvas>
	);
};

export default StarfieldEffect;
