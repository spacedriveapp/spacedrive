"""Convert MediaPipe BlazeFace full-range from TFLite to the ONNX file Spacedrive pins.

tf2onnx numbers the tensors it creates in a different order on each run, so
its raw output never hashes the same twice. This script renames every
internal tensor and node by its position in the graph, which makes the output
byte-identical across runs and machines with the pinned tool versions in
requirements-blazeface.txt.

    uv venv .venv-blazeface && . .venv-blazeface/bin/activate
    uv pip install -r scripts/models/requirements-blazeface.txt
    python scripts/models/convert_blazeface.py out/blaze_face_full_range.onnx

The source file and the result are checked against their pinned SHA-256s.
"""

import hashlib
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

import onnx

SOURCE_URL = (
    "https://huggingface.co/litert-community/MediaPipe-FaceDetector-BlazeFace-Full-Range"
    "/resolve/main/blaze_face_full_range.tflite"
)
SOURCE_SHA256 = "3698b18f063835bc609069ef052228fbe86d9c9a6dc8dcb7c7c2d69aed2b181b"
EXPECTED_SHA256 = "494c597f51002a1ccbdc3571cc3d02a60fea914a7ad8abf6968e31768adf422a"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonicalize(model: onnx.ModelProto) -> onnx.ModelProto:
    graph = model.graph
    keep = {v.name for v in graph.input} | {v.name for v in graph.output}
    names = {}

    def rename(name: str) -> str:
        if not name or name in keep:
            return name
        if name not in names:
            names[name] = f"t{len(names)}"
        return names[name]

    for index, node in enumerate(graph.node):
        node.name = f"n{index}"
        node.input[:] = [rename(n) for n in node.input]
        node.output[:] = [rename(n) for n in node.output]
    for init in graph.initializer:
        init.name = rename(init.name)
    for info in graph.value_info:
        info.name = rename(info.name)
    initializers = sorted(graph.initializer, key=lambda i: int(i.name[1:]))
    del graph.initializer[:]
    graph.initializer.extend(initializers)
    del graph.value_info[:]
    graph.doc_string = ""
    model.doc_string = ""
    model.producer_version = ""
    return model


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: convert_blazeface.py <output.onnx>")
    output = Path(sys.argv[1])
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        source = Path(tmp) / "blaze_face_full_range.tflite"
        urllib.request.urlretrieve(SOURCE_URL, source)
        if sha256(source) != SOURCE_SHA256:
            sys.exit(f"source hash mismatch: {sha256(source)}")
        raw = Path(tmp) / "raw.onnx"
        subprocess.run(
            [sys.executable, "-m", "tf2onnx.convert", "--tflite", str(source), "--output", str(raw), "--opset", "13"],
            check=True,
            capture_output=True,
        )
        model = canonicalize(onnx.load(raw))
        onnx.checker.check_model(model)
        output.write_bytes(model.SerializeToString(deterministic=True))
    digest = sha256(output)
    print(f"{output} {output.stat().st_size} bytes sha256 {digest}")
    if digest != EXPECTED_SHA256:
        sys.exit(f"expected sha256 {EXPECTED_SHA256}")


if __name__ == "__main__":
    main()
