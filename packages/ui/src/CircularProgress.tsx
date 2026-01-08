// adapted from https://github.com/martyan/react-customizable-progressbar/
import clsx from "clsx";
import type React from "react";
import { type FunctionComponent, useEffect, useState } from "react";

export type CircularProgressProps = {
  radius: number;
  progress: number;
  steps?: number;
  cut?: number;
  rotate?: number;
  strokeWidth?: number;
  strokeColor?: string;
  fillColor?: string;
  strokeLinecap?: "round" | "inherit" | "butt" | "square";
  transition?: string;
  pointerRadius?: number;
  pointerStrokeWidth?: number;
  pointerStrokeColor?: string;
  pointerFillColor?: string;
  trackStrokeColor?: string;
  trackStrokeWidth?: number;
  trackStrokeLinecap?: "round" | "inherit" | "butt" | "square";
  trackTransition?: string;
  counterClockwise?: boolean;
  inverse?: boolean;
  initialAnimation?: boolean;
  initialAnimationDelay?: number;
  className?: string;
  children?: React.ReactNode;
};

export const CircularProgress: FunctionComponent<CircularProgressProps> = ({
  radius,
  progress,
  steps = 100,
  cut = 0,
  rotate = -90,
  strokeWidth = 20,
  strokeColor = "indianred",
  fillColor = "none",
  strokeLinecap = "round",
  transition = ".3s ease",
  pointerRadius = 0,
  pointerStrokeWidth = 20,
  pointerStrokeColor = "indianred",
  pointerFillColor = "white",
  trackStrokeColor = "#e6e6e6",
  trackStrokeWidth = 20,
  trackStrokeLinecap = "round",
  trackTransition = ".3s ease",
  counterClockwise = false,
  inverse = false,
  initialAnimation = false,
  initialAnimationDelay = 0,
  className = "",
  children,
}) => {
  const [animationInitialized, setAnimationInitialized] = useState(false);

  useEffect(() => {
    if (initialAnimation) {
      const timeout = setTimeout(
        () => setAnimationInitialized(true),
        initialAnimationDelay
      );
      return () => clearTimeout(timeout);
    }
  }, [initialAnimation, initialAnimationDelay]);

  if (Number.isNaN(progress)) progress = 0;

  const getProgress = () =>
    initialAnimation && !animationInitialized ? 0 : progress;

  const circumference = radius * 2 * Math.PI;

  const strokeDasharray = `${circumference} ${circumference}`;
  const strokeDashoffset = ((100 - getProgress()) / 100) * circumference;

  // The space needed for the strokeWidth on all sides
  const fullStrokeWidth = strokeWidth * 2;

  // Adjust the svgSize to account for the space needed for the strokeWidth
  const svgSize = radius * 2 + fullStrokeWidth;
  const viewBox = `0 0 ${svgSize} ${svgSize}`;

  // Adjust the cx and cy to be the actual center of the SVG
  const center = radius + strokeWidth; // The center is radius + strokeWidth

  return (
    <div
      className={clsx("relative", className)}
      style={{
        width: `${svgSize}px`,
        height: `${svgSize}px`,
      }}
    >
      <svg
        height={svgSize}
        style={{ transform: `rotate(${rotate}deg)` }}
        viewBox={viewBox}
        width={svgSize}
      >
        {trackStrokeWidth > 0 && (
          <circle
            className="track-stroke"
            cx={center}
            cy={center}
            fill="none"
            r={radius}
            stroke={trackStrokeColor}
            strokeDasharray={strokeDasharray}
            strokeLinecap={trackStrokeLinecap}
            strokeWidth={trackStrokeWidth}
            style={{ transition: trackTransition }}
          />
        )}
        {strokeWidth > 0 && (
          <circle
            className="progress-stroke"
            cx={center}
            cy={center}
            fill={fillColor}
            r={radius}
            stroke={strokeColor}
            strokeDasharray={strokeDasharray}
            strokeLinecap={strokeLinecap}
            strokeWidth={strokeWidth}
            style={{ transition, strokeDashoffset }}
          />
        )}
        {pointerRadius > 0 && (
          <circle
            className="pointer-stroke"
            cx={radius}
            cy={radius}
            fill={pointerFillColor}
            r={pointerRadius}
            stroke={pointerStrokeColor}
            strokeWidth={pointerStrokeWidth}
            style={{
              transform: `rotate(${rotate}deg)`,
              transformOrigin: `${radius}px ${radius}px`,
            }}
          />
        )}
      </svg>
      {children}
    </div>
  );
};
