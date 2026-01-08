import type { File } from "@sd/ts-client";
import clsx from "clsx";
import { useEffect, useRef, useState } from "react";

interface TitleProps {
  file: File;
  editable?: boolean;
  selected?: boolean;
  onRename?: (newName: string) => void;
  className?: string;
}

export function Title({
  file,
  editable = false,
  selected = false,
  onRename,
  className,
}: TitleProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editValue, setEditValue] = useState(file.name);
  const ref = useRef<HTMLDivElement>(null);

  const handleBlur = () => {
    setIsEditing(false);
    if (editValue !== file.name && onRename) {
      onRename(editValue);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleBlur();
    }
    if (e.key === "Escape") {
      setEditValue(file.name);
      setIsEditing(false);
    }
  };

  const highlightText = () => {
    if (!ref.current) return;

    const endRange = file.name.lastIndexOf(".");
    const range = document.createRange();
    const node = ref.current.childNodes[0];

    if (node) {
      range.setStart(node, 0);
      range.setEnd(node, endRange > 0 ? endRange : file.name.length);

      const sel = window.getSelection();
      sel?.removeAllRanges();
      sel?.addRange(range);
    }
  };

  useEffect(() => {
    if (isEditing) {
      ref.current?.focus();
      highlightText();
    }
  }, [isEditing]);

  return (
    <div
      className={clsx(
        "cursor-default overflow-hidden rounded-md px-1.5 py-px text-ink text-xs outline-none",
        isEditing && "bg-app ring-2 ring-accent",
        !isEditing && "truncate",
        className
      )}
      contentEditable={isEditing}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
      ref={ref}
      suppressContentEditableWarning
    >
      {file.name}
      {file.extension && `.${file.extension}`}
    </div>
  );
}
