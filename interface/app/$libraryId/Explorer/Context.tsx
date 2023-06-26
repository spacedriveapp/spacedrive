import { createContext, useContext } from 'react';
import { FilePath, Location, NodeState, Tag } from '@sd/client';

export type ExplorerParent =
	| {
			type: 'Location';
			location: Location;
			subPath?: FilePath;
	  }
	| {
			type: 'Tag';
			tag: Tag;
	  }
	| {
			type: 'Node';
			node: NodeState;
	  };

interface ExplorerContext {
	parent?: ExplorerParent;
}

export const ExplorerContext = createContext<ExplorerContext | null>(null);

export const useExplorerContext = () => {
	const ctx = useContext(ExplorerContext);

	if (ctx === null) throw new Error('ExplorerContext.Provider not found!');

	return ctx;
};
