import { PropsWithChildren } from 'react';

import { Doc, DocsNavigation } from '../pages/docs/api';
import DocsSidebar from './DocsSidebar';

interface Props extends PropsWithChildren {
	doc?: Doc;
	navigation: DocsNavigation;
}

export default function DocsLayout(props: Props) {
	return (
		<div className="flex items-start w-full">
			<aside className="sticky mt-32 mb-20 top-32">
				<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
			</aside>
			<div className="w-full ">{props.children}</div>
		</div>
	);
}
