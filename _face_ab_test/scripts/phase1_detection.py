#!/usr/bin/env python3
"""Phase 1: Compare face detection backends on the same test images."""

import argparse
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent.parent))
from lib.common import (
    RESULTS_DIR,
    TEST_FILES,
    TimedOperation,
    collect_test_images,
    compute_detection_metrics,
    load_config,
    load_ground_truth,
    load_image,
    relative_path,
    save_result,
)

OUTPUT_FILE = RESULTS_DIR / "phase1_detection.json"


# --- Detector implementations ---


def detect_scrfd(img_rgb: np.ndarray, config: dict) -> list[dict]:
    """InsightFace SCRFD detector (buffalo_l model pack)."""
    from insightface.app import FaceAnalysis

    if not hasattr(detect_scrfd, "_app"):
        det_cfg = config.get("detection", {}).get("scrfd", {})
        det_size = tuple(det_cfg.get("det_size", [640, 640]))
        app = FaceAnalysis(name="buffalo_l", providers=["CUDAExecutionProvider", "CPUExecutionProvider"])
        app.prepare(ctx_id=0, det_size=det_size)
        detect_scrfd._app = app

    faces = detect_scrfd._app.get(img_rgb)
    results = []
    for face in faces:
        bbox = face.bbox.astype(int).tolist()  # [x1, y1, x2, y2]
        x1, y1, x2, y2 = bbox
        results.append({
            "bbox": [x1, y1, x2 - x1, y2 - y1],  # convert to [x, y, w, h]
            "confidence": round(float(face.det_score), 4),
        })
    return results


def detect_mtcnn(img_rgb: np.ndarray, config: dict) -> list[dict]:
    """MTCNN detector via facenet-pytorch."""
    from facenet_pytorch import MTCNN
    from PIL import Image

    if not hasattr(detect_mtcnn, "_detector"):
        det_cfg = config.get("detection", {}).get("mtcnn", {})
        detect_mtcnn._detector = MTCNN(
            keep_all=True,
            min_face_size=det_cfg.get("min_face_size", 20),
            thresholds=det_cfg.get("thresholds", [0.6, 0.7, 0.7]),
            device="cuda" if _cuda_available() else "cpu",
        )

    pil_img = Image.fromarray(img_rgb)
    boxes, probs = detect_mtcnn._detector.detect(pil_img)

    results = []
    if boxes is not None:
        for box, prob in zip(boxes, probs):
            x1, y1, x2, y2 = box.astype(int).tolist()
            results.append({
                "bbox": [x1, y1, x2 - x1, y2 - y1],
                "confidence": round(float(prob), 4),
            })
    return results


def detect_yolov8(img_rgb: np.ndarray, config: dict) -> list[dict]:
    """YOLOv8 detector via ultralytics — uses general yolov8n (person class 0) as face proxy.

    Note: A dedicated yolov8-face model requires a custom .pt file. The stock yolov8n
    detects "person" (class 0) which serves as a baseline comparison. For true face
    detection, swap the model path in BENCHMARK_CONFIG.json to a face-trained variant.
    """
    from ultralytics import YOLO

    if not hasattr(detect_yolov8, "_model"):
        det_cfg = config.get("detection", {}).get("yolov8_face", {})
        model_path = det_cfg.get("model", "yolov8n.pt")
        detect_yolov8._model = YOLO(model_path)

    preds = detect_yolov8._model(img_rgb, verbose=False, classes=[0])  # class 0 = person

    results = []
    for r in preds:
        for box in r.boxes:
            x1, y1, x2, y2 = box.xyxy[0].cpu().numpy().astype(int).tolist()
            conf = float(box.conf[0].cpu().numpy())
            results.append({
                "bbox": [x1, y1, x2 - x1, y2 - y1],
                "confidence": round(conf, 4),
            })
    return results


def detect_mediapipe(img_rgb: np.ndarray, config: dict) -> list[dict]:
    """MediaPipe Face Detection using the Tasks API (0.10.14+)."""
    import mediapipe as mp

    if not hasattr(detect_mediapipe, "_detector"):
        det_cfg = config.get("detection", {}).get("mediapipe", {})
        min_conf = det_cfg.get("min_detection_confidence", 0.5)

        base_options = mp.tasks.BaseOptions(model_asset_path=_get_mediapipe_model_path())
        options = mp.tasks.vision.FaceDetectorOptions(
            base_options=base_options,
            min_detection_confidence=min_conf,
            running_mode=mp.tasks.vision.RunningMode.IMAGE,
        )
        detect_mediapipe._detector = mp.tasks.vision.FaceDetector.create_from_options(options)

    mp_image = mp.Image(image_format=mp.ImageFormat.SRGB, data=img_rgb)
    result = detect_mediapipe._detector.detect(mp_image)

    results = []
    h, w = img_rgb.shape[:2]
    for det in result.detections:
        bb = det.bounding_box
        results.append({
            "bbox": [bb.origin_x, bb.origin_y, bb.width, bb.height],
            "confidence": round(float(det.categories[0].score), 4),
        })
    return results


def _get_mediapipe_model_path() -> str:
    """Download the MediaPipe BlazeFace short-range model if not cached."""
    import urllib.request
    cache_dir = Path.home() / ".cache" / "mediapipe"
    cache_dir.mkdir(parents=True, exist_ok=True)
    model_path = cache_dir / "blaze_face_short_range.tflite"
    if not model_path.exists():
        url = "https://storage.googleapis.com/mediapipe-models/face_detector/blaze_face_short_range/float16/latest/blaze_face_short_range.tflite"
        print(f"  Downloading MediaPipe face model to {model_path}...")
        urllib.request.urlretrieve(url, model_path)
    return str(model_path)


def _cuda_available() -> bool:
    try:
        import torch
        return torch.cuda.is_available()
    except ImportError:
        return False


DETECTORS = {
    "scrfd": detect_scrfd,
    "mtcnn": detect_mtcnn,
    "yolov8_face": detect_yolov8,
    "mediapipe": detect_mediapipe,
}


def main():
    parser = argparse.ArgumentParser(description="Phase 1: Face detection benchmark")
    parser.add_argument("--detectors", nargs="+", choices=list(DETECTORS.keys()),
                        default=list(DETECTORS.keys()),
                        help="Which detectors to benchmark (default: all)")
    parser.add_argument("--limit", type=int, default=0,
                        help="Max images to process (0 = all)")
    args = parser.parse_args()

    print("=" * 60)
    print("Phase 1: Face Detection Comparison")
    print("=" * 60)

    config = load_config()
    ground_truth = load_ground_truth()

    images = collect_test_images()
    if not images:
        print("ERROR: No test images found in test_files/")
        print("Place images in _face_ab_test/test_files/ and try again.")
        return 1

    if args.limit > 0:
        images = images[: args.limit]

    print(f"\nImages: {len(images)}")
    print(f"Detectors: {', '.join(args.detectors)}")
    if ground_truth:
        print("Ground truth: loaded")
    else:
        print("Ground truth: not available (metrics will be skipped)")

    results_by_detector = {}

    for det_name in args.detectors:
        detect_fn = DETECTORS[det_name]
        print(f"\n{'─' * 40}")
        print(f"Detector: {det_name}")
        print(f"{'─' * 40}")

        det_results = []
        total_faces = 0
        total_time = 0.0
        errors = []

        for img_path in images:
            rel = relative_path(img_path)
            try:
                img = load_image(img_path)
                with TimedOperation(f"{det_name} {img_path.name}") as t:
                    faces = detect_fn(img, config)

                entry = {
                    "file": rel,
                    "faces_found": len(faces),
                    "faces": faces,
                    "time_s": round(t.elapsed, 4),
                }

                # Compute metrics if ground truth available
                gt_key = img_path.name
                if ground_truth and gt_key in ground_truth.get("images", {}):
                    gt_faces = ground_truth["images"][gt_key]["faces"]
                    metrics = compute_detection_metrics(faces, gt_faces)
                    entry["metrics"] = metrics

                det_results.append(entry)
                total_faces += len(faces)
                total_time += t.elapsed

            except Exception as e:
                print(f"  ERROR: {rel} — {e}")
                errors.append({"file": rel, "error": str(e)})

        # Confidence distribution
        all_confs = [f["confidence"] for r in det_results for f in r["faces"]]
        conf_stats = {}
        if all_confs:
            conf_stats = {
                "min": round(min(all_confs), 4),
                "max": round(max(all_confs), 4),
                "mean": round(float(np.mean(all_confs)), 4),
                "median": round(float(np.median(all_confs)), 4),
            }

        # Per-threshold false positive analysis
        threshold_analysis = []
        for thresh in config.get("quality_filters", {}).get("min_confidence_thresholds", [0.3, 0.5, 0.7, 0.8]):
            filtered_counts = []
            for r in det_results:
                filtered = [f for f in r["faces"] if f["confidence"] >= thresh]
                filtered_counts.append(len(filtered))
            threshold_analysis.append({
                "threshold": thresh,
                "avg_faces_per_image": round(float(np.mean(filtered_counts)), 2) if filtered_counts else 0,
                "total_faces": sum(filtered_counts),
            })

        # Aggregate metrics
        agg_metrics = None
        entries_with_metrics = [r for r in det_results if "metrics" in r]
        if entries_with_metrics:
            agg_metrics = {
                "avg_precision": round(float(np.mean([r["metrics"]["precision"] for r in entries_with_metrics])), 4),
                "avg_recall": round(float(np.mean([r["metrics"]["recall"] for r in entries_with_metrics])), 4),
                "avg_f1": round(float(np.mean([r["metrics"]["f1"] for r in entries_with_metrics])), 4),
            }

        results_by_detector[det_name] = {
            "total_images": len(det_results),
            "total_faces_found": total_faces,
            "avg_faces_per_image": round(total_faces / len(det_results), 2) if det_results else 0,
            "total_time_s": round(total_time, 4),
            "avg_time_per_image_s": round(total_time / len(det_results), 4) if det_results else 0,
            "confidence_distribution": conf_stats,
            "threshold_analysis": threshold_analysis,
            "aggregate_metrics": agg_metrics,
            "errors": errors,
            "files": det_results,
        }

        print(f"\n  Summary: {total_faces} faces in {len(det_results)} images, "
              f"avg {total_time / len(det_results):.4f}s/image" if det_results else "")
        if agg_metrics:
            print(f"  Avg P={agg_metrics['avg_precision']:.3f} R={agg_metrics['avg_recall']:.3f} "
                  f"F1={agg_metrics['avg_f1']:.3f}")

    output = {
        "phase": "1_detection",
        "total_images": len(images),
        "detectors": results_by_detector,
    }

    save_result(output, OUTPUT_FILE)

    # Final comparison table
    print(f"\n{'=' * 60}")
    print("Detection Comparison Summary")
    print(f"{'=' * 60}")
    print(f"{'Detector':<15} {'Faces':>6} {'Avg/img':>8} {'Avg time':>10} {'F1':>6}")
    print(f"{'─' * 15} {'─' * 6} {'─' * 8} {'─' * 10} {'─' * 6}")
    for name, r in results_by_detector.items():
        f1 = r["aggregate_metrics"]["avg_f1"] if r.get("aggregate_metrics") else "—"
        f1_str = f"{f1:.3f}" if isinstance(f1, float) else f1
        print(f"{name:<15} {r['total_faces_found']:>6} {r['avg_faces_per_image']:>8.2f} "
              f"{r['avg_time_per_image_s']:>9.4f}s {f1_str:>6}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
