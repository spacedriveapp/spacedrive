import { useExplorer } from "../../context";
import { FileRow } from "./FileRow";

export function ListView() {
  const { files, selectedFiles, selectFile } = useExplorer();

  return (
    <div className="flex flex-col p-6">
      <div className="flex items-center px-2 py-1 text-xs font-semibold text-ink-dull border-b border-app-line mb-2">
        <div className="w-10"></div>
        <div className="flex-1">Name</div>
        <div className="w-24">Size</div>
        <div className="w-32">Modified</div>
        <div className="w-24">Type</div>
      </div>

      {files.map((file) => (
        <FileRow
          key={file.id}
          file={file}
          selected={selectedFiles.some((f) => f.id === file.id)}
          onSelect={selectFile}
        />
      ))}
    </div>
  );
}
