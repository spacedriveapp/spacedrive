'use client';

import clsx from 'clsx';
import Image from 'next/image';
import React from 'react';

const WormHole = () => {
	return (
		<div
			className="relative mb-[225px] mt-[240px] flex w-full max-w-[800px] items-center justify-center sm:mb-[220px]
					 sm:mt-[250px] md:mb-[280px] md:mt-[340px] lg:my-[400px]"
		>
			<div
				className="absolute top-[-150px] w-full max-w-[450px] rotate-[300deg] sm:top-[-200px]
						 sm:max-w-[500px] md:top-[-200px] lg:top-auto lg:mr-[250px] lg:max-w-full lg:rotate-0"
			>
				<div className="absolute left-[200px] top-[50px] z-10 size-full">
					<Image
						width={30}
						height={45}
						quality={100}
						alt="heart"
						className="heart"
						src="/images/icons/heart.svg"
					/>
				</div>
				<div className="absolute left-[200px] top-[50px] z-10 size-full">
					<Image
						width={40}
						height={45}
						quality={100}
						alt="game"
						className="game"
						src="/images/icons/game.svg"
					/>
				</div>
				<div
					className="absolute top-[-100px] z-10
				size-full sm:left-[200px]
				sm:top-[10px]"
				>
					<Image
						width={40}
						height={45}
						quality={100}
						alt="image"
						className="image"
						src="/images/icons/image.svg"
					/>
				</div>
				<div
					className="absolute left-[120px] top-[-50px]
				z-10 size-full sm:left-[200px]
				sm:top-[-10px]"
				>
					<Image
						width={40}
						height={45}
						quality={100}
						alt="lock"
						className="lock"
						src="/images/icons/lock.svg"
					/>
				</div>
				<div
					className="absolute left-[200px] top-[350px] z-10 size-full
				 lg:left-[200px] lg:top-[300px]"
				>
					<Image
						width={40}
						height={45}
						quality={100}
						alt="video"
						className="videoicon"
						src="/images/icons/video.svg"
					/>
				</div>
				<div className="absolute left-[200px] top-[150px] z-10 size-full">
					<Image
						width={40}
						height={45}
						quality={100}
						alt="application"
						className="appicon"
						src="/images/icons/application.svg"
					/>
				</div>
				<div className="absolute left-[120px] top-[50px] z-10 size-full lg:left-[200px] lg:top-[120px]">
					<Image
						width={40}
						height={45}
						quality={100}
						alt="collection"
						className="collection"
						src="/images/icons/collection.svg"
					/>
				</div>
				<div className="absolute left-[120px] top-[50px] z-10 size-full sm:left-[200px] sm:top-[300px] lg:left-[200px] lg:top-[420px]">
					<Image
						width={40}
						height={45}
						quality={100}
						alt="node"
						className="node"
						src="/images/icons/node.svg"
					/>
				</div>
				<div
					className="absolute
					left-[60px] top-[-190px]
				z-10 size-full sm:left-[50px] sm:top-[50px] lg:left-[200px] lg:top-[490px]"
				>
					<Image
						width={40}
						height={45}
						quality={100}
						alt="texturedmesh"
						className="texturedmesh"
						src="/images/icons/texturedmesh.png"
					/>
				</div>
				<div
					className="absolute left-[120px] top-[50px]
				 z-10 size-full md:left-[200px] md:top-[350px]"
				>
					<Image
						width={40}
						height={45}
						quality={100}
						alt="database"
						className="database"
						src="/images/icons/database.svg"
					/>
				</div>
				<div className="absolute left-[100px] top-[-200px] z-10 size-full sm:left-[150px] sm:top-[50px]">
					<Image
						width={40}
						height={45}
						quality={100}
						alt="package"
						className="package"
						src="/images/icons/package.svg"
					/>
				</div>
				<Image
					loading="eager"
					width={1500}
					height={626}
					quality={100}
					alt="wormhole"
					src="/images/misc/wormhole.webp"
				/>
			</div>
			<div
				className={clsx(
					'worm-hole-border-gradient relative top-[100px] z-20 flex w-full max-w-[500px] flex-col rounded-lg',
					'items-center justify-center gap-2 bg-gradient-to-r from-[#080710]/0 to-[#080710]/50 p-8 backdrop-blur-sm'
				)}
			>
				<h1 className="bg-gradient-to-r from-white to-indigo-300 bg-clip-text text-[20px] font-bold text-transparent">
					Heading
				</h1>
				<p className="text-center text-sm text-gray-400">
					Lorem ipsum dolor sit amet consectetur adipisicing elit. Nam, iure ea dolores
					atque unde fugit ad libero debitis nemo quis culpa sequi illum aliquam iusto
					harum quo laborum ducimus voluptas Lorem, ipsum dolor sit amet consectetur
					adipisicing elit. Similique eos, voluptatum, ipsam facilis placeat tempore
					consequuntur officia distinctio voluptate blanditiis tenetur, animi ut ea
					laboriosam laborum culpa autem accusantium reprehenderit!
				</p>
			</div>
		</div>
	);
};

export default WormHole;
