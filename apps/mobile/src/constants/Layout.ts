import { Dimensions } from 'react-native';

const { width, height } = Dimensions.get('window');

export default {
	window: {
		width,
		height
	},
	isSmallDevice: width < 375
};
