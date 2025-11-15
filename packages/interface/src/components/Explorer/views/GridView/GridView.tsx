import { useExplorer } from "../../context";
import { FileCard } from "./FileCard";

export function GridView() {
  const { files, selectedFiles, selectFile, viewSettings, focusedIndex } =
    useExplorer();
  const { gridSize, gapSize } = viewSettings;

  return (
    <div
      className="grid p-3"
      style={{
        gridTemplateColumns: `repeat(auto-fill, minmax(${gridSize}px, 1fr))`,
        gap: `${gapSize}px`,
      }}
    >
      {files.map((file, index) => (
        <FileCard
          key={file.id}
          file={file}
          selected={selectedFiles.some((f) => f.id === file.id)}
          focused={index === focusedIndex}
          onSelect={selectFile}
        />
      ))}
    </div>
  );
}
