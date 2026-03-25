import { Navigate, Outlet, type RouteObject } from 'react-router-dom';
import { SpacebotLayout } from './SpacebotLayout';
import { ChatRoute } from './routes/ChatRoute';
import { ConversationRoute } from './routes/ConversationRoute';
import { TasksRoute } from './routes/TasksRoute';
import { MemoriesRoute } from './routes/MemoriesRoute';
import { AutonomyRoute } from './routes/AutonomyRoute';
import { ScheduleRoute } from './routes/ScheduleRoute';

/**
 * Spacebot nested route configuration
 * These routes are mounted under /spacebot in the main router
 */
export const spacebotRoutes: RouteObject[] = [
	{
		path: 'spacebot',
		element: <SpacebotLayout />,
		children: [
			{
				index: true,
				element: <Navigate to="/spacebot/chat" replace />,
			},
			{
				path: 'chat',
				children: [
					{
						index: true,
						element: <ChatRoute />,
					},
					{
						path: 'new',
						element: <ChatRoute />,
					},
					{
						path: 'conversation/:conversationId',
						element: <ConversationRoute />,
					},
				],
			},
			{
				path: 'tasks',
				element: <TasksRoute />,
			},
			{
				path: 'memories',
				element: <MemoriesRoute />,
			},
			{
				path: 'autonomy',
				element: <AutonomyRoute />,
			},
			{
				path: 'schedule',
				element: <ScheduleRoute />,
			},
		],
	},
];

/**
 * Spacebot Router Provider component that wraps the Outlet
 * Used when Spacebot routes are mounted within the main router
 */
export function SpacebotRouter() {
	return <Outlet />;
}
