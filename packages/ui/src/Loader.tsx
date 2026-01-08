import clsx from "clsx";
import { Puff } from "react-loading-icons";

export function Loader(props: { className?: string; color?: string }) {
  return (
    <Puff
      className={clsx("size-7", props.className)}
      speed={1}
      stroke={props.color || "#2599FF"}
      strokeOpacity={4}
      strokeWidth={5}
    />
  );
}
