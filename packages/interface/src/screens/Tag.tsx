import React, { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

export const TagScreen: React.FC<{}> = () => {
	let [searchParams] = useSearchParams();
	let path = searchParams.get('path') || '';

	let { id } = useParams();

	return <div className="p-5 text-gray-450">Tag screen coming soon...</div>;
};
