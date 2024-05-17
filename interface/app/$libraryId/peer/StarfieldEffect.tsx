import React, { useEffect, useRef } from 'react';

const StarfieldEffect: React.FC = () => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const resizeCanvas = () => {
      canvas.width = canvas.parentElement?.clientWidth || 800;
      canvas.height = canvas.parentElement?.clientHeight || 300;
    };

    resizeCanvas();

    window.addEventListener('resize', resizeCanvas);

    canvas.style.position = 'absolute';
    canvas.oncontextmenu = e => e.preventDefault();

    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const pix = imageData.data;

    const center = { x: canvas.width / 2, y: canvas.height / 2 };

    let mouseActive = false;
    let fov = 300;
    const fovMin = 210;
    const fovMax = fov;

    const starHolderCount = 2000;
    const starHolder: any[] = [];
    const starBgHolder: any[] = [];
    let starSpeed = 10;
    const starSpeedMin = starSpeed;
    const starSpeedMax = 100;
    const starDistance = 8000;

    const backgroundColor = { r: 29, g: 28, b: 38, a: 255 };

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

    const setPixelAdditive = (x: number, y: number, r: number, g: number, b: number, a: number) => {
      const i = (x + y * canvas.width) * 4;
      pix[i] += r;
      pix[i + 1] += g;
      pix[i + 2] += b;
      pix[i + 3] = a;
    };

    const drawLine = (x1: number, y1: number, x2: number, y2: number, r: number, g: number, b: number, a: number) => {
      const dx = Math.abs(x2 - x1);
      const dy = Math.abs(y2 - y1);
      const sx = x1 < x2 ? 1 : -1;
      const sy = y1 < y2 ? 1 : -1;
      let err = dx - dy;
      let lx = x1;
      let ly = y1;

      while (true) {
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

    const addParticle = (x: number, y: number, z: number, ox: number, oy: number, oz: number) => {
      const particle = { x, y, z, ox, oy, x2d: 0, y2d: 0 };
      return particle;
    };

    const addParticles = () => {
      let x, y, z, colorValue, particle;
      for (let i = 0; i < starHolderCount / 3; i++) {
        x = Math.random() * 24000 - 12000;
        y = Math.random() * 4500 - 2250;
        z = Math.round(Math.random() * starDistance);
        colorValue = 255;
        particle = addParticle(x, y, z, x, y, z);
        particle.color = { r: colorValue, g: colorValue, b: colorValue, a: 255 };
        starBgHolder.push(particle);
      }
      for (let i = 0; i < starHolderCount; i++) {
        x = Math.random() * 10000 - 5000;
        y = Math.random() * 10000 - 5000;
        z = Math.round(Math.random() * starDistance);
        colorValue = 255;
        particle = addParticle(x, y, z, x, y, z);
        particle.color = { r: colorValue, g: colorValue, b: colorValue, a: 255 };
        particle.oColor = { r: colorValue, g: colorValue, b: colorValue, a: 255 };
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
        if (star.x2d > 0 && star.x2d < canvas.width && star.y2d > 0 && star.y2d < canvas.height) {
          setPixel(star.x2d | 0, star.y2d | 0, star.color.r, star.color.g, star.color.b, 255);
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

        if (star.x2d > 0 && star.x2d < canvas.width && star.y2d > 0 && star.y2d < canvas.height) {
          setPixelAdditive(star.x2d | 0, star.y2d | 0, star.color.r, star.color.g, star.color.b, 255);
        }

        if (starSpeed !== starSpeedMin) {
          const nz = star.z + warpSpeedValue;
          scale = fov / (fov + nz);
          const x2d = star.x * scale + center.x;
          const y2d = star.y * scale + center.y;
          if (x2d > 0 && x2d < canvas.width && y2d > 0 && y2d < canvas.height) {
            drawLine(star.x2d | 0, star.y2d | 0, x2d | 0, y2d | 0, star.color.r, star.color.g, star.color.b, 255);
          }
        }
      }

      ctx.putImageData(imageData, 0, 0);
    };

    addParticles();
    animloop();

    const mouseHandler = (e: MouseEvent) => {
      mouseActive = true;
      mousePos = { x: e.clientX - center.x, y: e.clientY - center.y };
    };

    window.addEventListener('mousemove', mouseHandler);

    window.addEventListener('mousedown', () => {
      mouseDown = true;
    });

    window.addEventListener('mouseup', () => {
      mouseDown = false;
      mouseActive = false;
    });

    return () => {
      window.removeEventListener('mousemove', mouseHandler);
      window.removeEventListener('resize', resizeCanvas);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="block size-full rounded-lg border border-gray-500 hover:scale-105"
    >
    </canvas>
  );
};

export default StarfieldEffect;
