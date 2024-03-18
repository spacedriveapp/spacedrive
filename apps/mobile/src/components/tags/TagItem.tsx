import { DotsThreeOutlineVertical, List } from "phosphor-react-native";
import { View, Pressable } from "react-native";
import { Swipeable } from "react-native-gesture-handler";
import { ClassInput } from "twrnc";
import { twStyle, tw } from "~/lib/tailwind";
import Card from "../layout/Card";
import { TagModal } from "../modal/tag/TagModal";
import RightActions from "./RightActions";
import { Tag } from "@sd/client";
import { Text } from "react-native";
import { useRef } from "react";
import { ModalRef } from "../layout/Modal";
import GridTagView from "./GridTagView";
import ListTagView from "./ListTagView";

type TagItemProps = {
	tag: Tag
	onPress: () => void;
	tagStyle?: ClassInput;
	viewStyle?: 'grid' | 'list';
};

export const TagItem = ({ tag, onPress, tagStyle, viewStyle = 'grid' }: TagItemProps) => {
	const modalRef = useRef<ModalRef>(null);
	return (
		<Pressable onPress={onPress} testID="browse-tag">
			{viewStyle === 'grid' ? (
				<GridTagView tag={tag} tagStyle={tagStyle} modalRef={modalRef} />
			) : (
			<ListTagView tag={tag} modalRef={modalRef} />
			)}
			<TagModal ref={modalRef} tag={tag} />
		</Pressable>
	);
};
