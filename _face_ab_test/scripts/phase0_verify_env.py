#!/usr/bin/env python3
"""Phase 0: Verify environment — check dependencies, GPU, models, and test files."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))
from lib.common import TEST_FILES, collect_test_images

passed_count = 0
failed_count = 0
warnings_count = 0


def check(name: str, ok: bool, detail: str = "") -> bool:
    global passed_count, failed_count
    status = "PASS" if ok else "FAIL"
    msg = f"  [{status}] {name}"
    if detail:
        msg += f" — {detail}"
    print(msg)
    if ok:
        passed_count += 1
    else:
        failed_count += 1
    return ok


def warn(name: str, detail: str = "") -> None:
    global warnings_count
    msg = f"  [WARN] {name}"
    if detail:
        msg += f" — {detail}"
    print(msg)
    warnings_count += 1


def main():
    print("=" * 60)
    print("Phase 0: Environment Verification")
    print("=" * 60)

    # --- Python version ---
    print("\n--- Python ---")
    v = sys.version_info
    check("Python >= 3.10", v >= (3, 10), f"{v.major}.{v.minor}.{v.micro}")

    # --- Core libraries ---
    print("\n--- Core Libraries ---")

    try:
        import numpy as np
        check("numpy", True, np.__version__)
    except ImportError:
        check("numpy", False, "not installed")

    try:
        import cv2
        check("opencv", True, cv2.__version__)
    except ImportError:
        check("opencv", False, "not installed (opencv-python-headless)")

    try:
        from PIL import Image
        import PIL
        check("Pillow", True, PIL.__version__)
    except ImportError:
        check("Pillow", False, "not installed")

    try:
        import sklearn
        check("scikit-learn", True, sklearn.__version__)
    except ImportError:
        check("scikit-learn", False, "not installed")

    # --- Face Detection Libraries ---
    print("\n--- Face Detection Libraries ---")

    try:
        import insightface
        check("insightface", True, insightface.__version__)
    except ImportError:
        check("insightface", False, "not installed")

    try:
        from facenet_pytorch import MTCNN
        check("facenet-pytorch (MTCNN)", True)
    except ImportError:
        check("facenet-pytorch", False, "not installed")

    try:
        from ultralytics import YOLO
        check("ultralytics (YOLOv8)", True)
    except ImportError:
        check("ultralytics", False, "not installed")

    try:
        import mediapipe as mp
        check("mediapipe", True, mp.__version__)
    except ImportError:
        check("mediapipe", False, "not installed")

    # --- Face Embedding Libraries ---
    print("\n--- Face Embedding Libraries ---")

    try:
        from facenet_pytorch import InceptionResnetV1
        check("facenet-pytorch (InceptionResnetV1)", True)
    except ImportError:
        check("facenet-pytorch (embeddings)", False, "not installed")

    try:
        from deepface import DeepFace
        check("deepface", True)
    except (ImportError, ValueError) as e:
        check("deepface", False, f"not usable — {e}")

    # --- GPU ---
    print("\n--- GPU / Acceleration ---")

    cuda_available = False
    try:
        import torch
        cuda_available = torch.cuda.is_available()
        if cuda_available:
            gpu_name = torch.cuda.get_device_name(0)
            check("PyTorch CUDA", True, f"{torch.__version__} — {gpu_name}")
        else:
            warn("PyTorch CUDA", f"{torch.__version__} — CPU only (slower but works)")
    except ImportError:
        warn("PyTorch", "not installed (some models need it)")

    try:
        import onnxruntime as ort
        providers = ort.get_available_providers()
        has_cuda = "CUDAExecutionProvider" in providers
        if has_cuda:
            check("ONNX Runtime GPU", True, f"{ort.__version__} — CUDA available")
        else:
            warn("ONNX Runtime GPU", f"{ort.__version__} — CPU only ({', '.join(providers)})")
    except ImportError:
        warn("ONNX Runtime", "not installed")

    # --- InsightFace Model Pack ---
    print("\n--- Model Availability ---")

    try:
        import insightface
        from insightface.app import FaceAnalysis

        app = FaceAnalysis(name="buffalo_l", providers=["CPUExecutionProvider"])
        app.prepare(ctx_id=-1, det_size=(640, 640))
        check("InsightFace buffalo_l model pack", True, "loaded successfully")
    except Exception as e:
        check(
            "InsightFace buffalo_l model pack",
            False,
            f"failed to load — {e}. Run: python -c \"from insightface.app import FaceAnalysis; FaceAnalysis(name='buffalo_l')\"",
        )

    # --- Test Files ---
    print("\n--- Test Files ---")

    if TEST_FILES.exists():
        images = collect_test_images()
        check("test_files/ directory", True, f"{len(images)} images found")
        if len(images) == 0:
            warn("No test images", "Place images in _face_ab_test/test_files/")
    else:
        check("test_files/ directory", False, "directory does not exist — create it and add test images")

    # --- Summary ---
    print("\n" + "=" * 60)
    total = passed_count + failed_count
    print(f"Results: {passed_count}/{total} passed, {failed_count} failed, {warnings_count} warnings")
    if failed_count == 0:
        print("Environment is ready!")
    else:
        print("Fix the failed checks above before running benchmarks.")
    print("=" * 60)

    return 0 if failed_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
