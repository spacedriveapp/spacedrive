import React from 'react';

const AccessData = () => {
	return (
		<div className="mb-[150px] md:mb-[200px]">
			<div className="relative h-0 pb-[100%]">
				<iframe
					src="https://my.spline.design/untitled-24fd6d26bb7545a00096266a37bb8f1b/"
					width={'100%'}
					height={'100%'}
					className="pointer-events-none absolute left-0 top-0 h-full w-full"
				/>
			</div>
			<div className="relative bottom-[60px] md:bottom-[140px]">
				<h3
					className="bg-gradient-to-r from-white to-violet-400 bg-clip-text text-center
						 text-[25px] font-bold text-transparent md:text-[30px]"
				>
					Access data from anywhere
				</h3>
				<p className="mx-auto w-full max-w-[800px] text-center text-sm text-ink-faint md:text-lg">
					users can enjoy the freedom of accessing their important files, documents, and
					media assets from any device with an internet connection, ensuring productivity
					and convenience on the go.
				</p>
			</div>
		</div>
	);
};

export default AccessData;
