import {
	DrawerNavigationState,
	ParamListBase,
	getFocusedRouteNameFromRoute
} from '@react-navigation/native';
import { proxy } from 'valtio';

export const currentLibraryStore = proxy({
	id: null as string | null
});

export const getActiveRouteFromState = function (state: any) {
	if (!state.routes || state.routes.length === 0 || state.index >= state.routes.length) {
		return state;
	}
	const childActiveRoute = state.routes[state.index];
	return getActiveRouteFromState(childActiveRoute);
};

export const getStackNameFromState = function (state: DrawerNavigationState<ParamListBase>) {
	return getFocusedRouteNameFromRoute(getActiveRouteFromState(state)) ?? 'OverviewStack';
};
