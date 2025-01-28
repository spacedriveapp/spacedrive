// @ts-check
const { withContentlayer } = require('next-contentlayer2');
const withBundleAnalyzer = require('@next/bundle-analyzer')({
	enabled: process.env.ANALYZE === 'true'
});
const { withPlausibleProxy } = require('next-plausible');

// Validate env on build // TODO: I wish we could do this so Vercel can warn us when we are wrong but it's too hard.
// import './src/env.mjs';

/** @type {import('next').NextConfig} */
const nextConfig = {
	reactStrictMode: true,
	swcMinify: true,
	transpilePackages: ['@sd/ui'],
	eslint: {
		ignoreDuringBuilds: true
	},
	typescript: {
		ignoreBuildErrors: true
	},
	experimental: {
		optimizePackageImports: ['@sd/ui']
	},
	headers: async () => [
		{
			source: '/api/:path*',
			headers: [
				{
					key: 'Access-Control-Allow-Origin',
					value: '*'
				},
				{
					key: 'Access-Control-Allow-Methods',
					value: 'GET, HEAD, OPTIONS'
				},
				{
					key: 'Access-Control-Allow-Headers',
					value: 'Content-Type'
				}
			]
		}
	],
	webpack(config) {
		// Grab the existing rule that handles SVG imports
		const fileLoaderRule = config.module.rules.find((rule) => rule.test?.test?.('.svg'));

		config.module.rules.push(
			// Reapply the existing rule, but only for svg imports ending in ?url
			{
				...fileLoaderRule,
				test: /\.svg$/i,
				resourceQuery: /url/ // *.svg?url
			},
			// Convert all other *.svg imports to React components so it's compatible with Vite's plugin.
			{
				test: /\.svg$/i,
				issuer: { not: /\.(css|scss|sass)$/ },
				resourceQuery: { not: /url/ }, // exclude if *.svg?url
				use: [
					{
						loader: '@svgr/webpack',
						options: {
							icon: true,
							exportType: 'named',
							typescript: true,
							svgProps: { fill: 'currentColor' }
						}
					}
				]
			}
		);

		// Modify the file loader rule to ignore *.svg, since we have it handled now.
		fileLoaderRule.exclude = /\.svg$/i;

		return config;
	}
};

module.exports = withPlausibleProxy()(withBundleAnalyzer(withContentlayer(nextConfig)));
