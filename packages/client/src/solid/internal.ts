import { useState } from 'react';

export function insideReactRender() {
	try {
		// eslint-disable-next-line react-hooks/rules-of-hooks
		useState();
		return true;
	} catch (err) {
		return false;
	}
}
