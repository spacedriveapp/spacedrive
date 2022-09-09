import { PropsWithChildren } from 'react';

import { Doc, DocItem, DocsList, SidebarCategory } from '../pages/docs/api';
import DocsSidebar from './DocsSidebar';

interface Props extends PropsWithChildren {
	doc?: Doc;
	docsList: DocsList;
}

export default function DocsLayout(props: Props) {
	return (
		<div className="flex items-start w-full">
			<aside className="sticky mt-32 mb-20 top-32">
				<DocsSidebar activePath={props?.doc?.url} data={props.docsList} />
			</aside>
			<div className="w-full ">{props.children}</div>
		</div>
	);
}
