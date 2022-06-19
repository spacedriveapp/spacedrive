import React, { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

export const TagScreen: React.FC<{}> = () => {
	let [searchParams] = useSearchParams();
	let path = searchParams.get('path') || '';

	let { id } = useParams();

	return (
		<div className="w-full p-5">
			<h1>{id}</h1>
		</div>
	);
};
