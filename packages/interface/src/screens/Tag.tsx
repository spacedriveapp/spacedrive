import React from 'react';
import { useParams } from 'react-router-dom';

export const TagScreen: React.FC<{}> = () => {
	let { id } = useParams();

	return (
		<div className="w-full p-5">
			<h1>{id}</h1>
		</div>
	);
};
