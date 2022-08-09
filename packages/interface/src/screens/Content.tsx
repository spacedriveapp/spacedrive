import React from 'react';

export const ContentScreen: React.FC<{}> = (props) => {
	// const [address, setAddress] = React.useState('');
	return (
		<div className="flex flex-col w-full h-screen p-5 custom-scroll page-scroll">
			{/* <div className="relative flex flex-col space-y-5 pb-7">
				<LockClosedIcon className="absolute w-4 h-4 ml-3 text-gray-250 top-[30px]" />
				<Input
					className="pl-9"
					placeholder="0f2z49zA"
					value={address}
					onChange={(e) => setAddress(e.target.value)}
				/>
			</div> */}
		</div>
	);
};
