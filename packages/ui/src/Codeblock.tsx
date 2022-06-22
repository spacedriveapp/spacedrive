import React from 'react';
import ReactJson, { ReactJsonViewProps } from 'react-json-view';

export interface CodeBlockProps extends ReactJsonViewProps {}

export default function CodeBlock(props: CodeBlockProps) {
	return (
		<ReactJson
			enableClipboard={false}
			displayDataTypes={false}
			theme="ocean"
			style={{
				padding: 20,
				borderRadius: 5,
				backgroundColor: '#101016',
				border: 1,
				borderColor: '#1E1E27',
				borderStyle: 'solid'
			}}
			{...props}
		/>
	);
}
