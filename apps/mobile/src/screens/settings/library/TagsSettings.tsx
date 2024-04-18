import Tags from '~/screens/browse/Tags';
import { ScrollY } from '~/types/shared';

const TagsSettingsScreen = ({ scrollY }: ScrollY) => {
	return <Tags scrollY={scrollY} viewStyle="list" />;
};

export default TagsSettingsScreen;
