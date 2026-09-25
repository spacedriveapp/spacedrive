import {createBrowserRouter, Navigate} from 'react-router-dom';
import {JobsScreen} from './components/JobManager';
import {AnalyzerView} from './routes/analyzer';
import {CollectionView} from './routes/collection';
import {DaemonManager} from './routes/daemon';
import {ExplorerView} from './routes/explorer';
import {RecentsView} from './routes/explorer/views/RecentsView';
import {FileKindsView} from './routes/file-kinds';
import {Overview} from './routes/overview';
import {RedundancyDashboard} from './routes/redundancy';
import {AtRiskFiles} from './routes/redundancy/at-risk';
import {CompareVolumes} from './routes/redundancy/compare';
import {DuplicatesView} from './routes/redundancy/duplicates';
import {TrashView} from './routes/trash';
import {SourcesHome} from './routes/sources';
import {AdaptersScreen} from './routes/sources/Adapters';
import {SourceDetail} from './routes/sources/SourceDetail';
import {TagView} from './routes/tag';
import {ShellLayout} from './ShellLayout';
/**
 * Router routes configuration (without router instance)
 */
export const explorerRoutes = [
	{
		path: '/',
		element: <ShellLayout />,
		children: [
			{
				index: true,
				element: <Overview />
			},
			{
				path: 'explorer',
				element: <ExplorerView />
			},
			{
				path: 'favorites',
				element: (
					<div className="text-ink flex h-full items-center justify-center">
						Favorites (coming soon)
					</div>
				)
			},
			{
				path: 'recents',
				element: <RecentsView />
			},
			{
				path: 'file-kinds',
				element: <FileKindsView />
			},
			{
				path: 'collection/:slug',
				element: <CollectionView />
			},
			{
				path: 'analyzer',
				element: <AnalyzerView />
			},
			{
				path: 'tag/:tagId',
				element: <TagView />
			},
			{
				path: 'sources',
				element: <SourcesHome />
			},
			{
				path: 'sources/adapters',
				element: <AdaptersScreen />
			},
			{
				path: 'sources/:sourceId',
				element: <SourceDetail />
			},
			{
				path: 'trash',
				element: <TrashView />
			},
			{
				path: 'redundancy',
				children: [
					{
						index: true,
						element: <RedundancyDashboard />
					},
					{
						path: 'at-risk',
						element: <AtRiskFiles />
					},
					{
						path: 'compare',
						element: <CompareVolumes />
					}
				]
			},
			{
				path: 'search',
				element: (
					<div className="text-ink flex h-full items-center justify-center">
						Search (coming soon)
					</div>
				)
			},
			{
				path: 'activity',
				element: <JobsScreen />
			},
			{
				path: 'jobs',
				element: <Navigate to="/activity" replace />
			},
			{
				path: 'daemon',
				element: <DaemonManager />
			}
		]
	}
];

/**
 * Router for the main Explorer interface
 */
export function createExplorerRouter(): ReturnType<typeof createBrowserRouter> {
	return createBrowserRouter(explorerRoutes);
}
