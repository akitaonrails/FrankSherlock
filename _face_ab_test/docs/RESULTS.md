# Face Recognition A/B Test Results

## Test Setup

- **Images**: 43 personal photos (mix of selfies, group shots, portraits)
- **Hardware**: CPU-only (ONNX Runtime fell back to CPUExecutionProvider, libcudnn.so.9 not available)
- **Date**: 2026-02-26

## Phase 1: Detection

### Detection Counts & Speed

| Detector | Faces Found | Avg/img | Total Time | Per Image |
|----------|------------:|--------:|-----------:|----------:|
| MTCNN | 191 | 4.44 | 20.51s | 0.477s |
| YOLOv8 (person) | 150 | 3.49 | 0.95s | 0.022s |
| SCRFD | 146 | 3.40 | 20.26s | 0.471s |
| MediaPipe | 36 | 0.84 | 3.34s | 0.078s |

### Confidence Distribution

| Detector | Min | Max | Mean | Median |
|----------|----:|----:|-----:|-------:|
| MTCNN | 0.713 | 1.000 | 0.950 | 1.000 |
| MediaPipe | 0.520 | 0.972 | 0.845 | 0.901 |
| SCRFD | 0.511 | 0.921 | 0.807 | 0.842 |
| YOLOv8 | 0.252 | 0.952 | 0.607 | 0.623 |

### Faces Retained at Confidence Thresholds

| Detector | >=0.3 | >=0.5 | >=0.7 | >=0.8 |
|----------|------:|------:|------:|------:|
| SCRFD | 146 | 146 | 122 | 101 |
| MTCNN | 191 | 191 | 191 | 163 |
| YOLOv8 | 133 | 96 | 60 | 44 |
| MediaPipe | 36 | 36 | 30 | 26 |

### Detection Analysis

- **MTCNN** finds the most faces (191) with the highest confidence (median 1.0) but is the slowest at ~0.48s/img. The high count likely includes some false positives.
- **SCRFD** is the most reliable face-specific detector — 146 faces with solid confidence (median 0.84), same speed as MTCNN but with built-in ArcFace embeddings (zero additional cost for recognition).
- **YOLOv8** is 20x faster (0.02s/img) but detects full persons, not faces — bounding boxes are not usable for face recognition/embedding without a face-trained model variant.
- **MediaPipe** is fast but only finds 36 faces — it's a short-range detector and misses small/distant faces entirely.

## Phase 2: Embedding

All three models produce 512-dimensional embeddings for the 146 SCRFD-detected faces.

| Model | Time/face | Notes |
|-------|----------:|-------|
| ArcFace (InsightFace) | ~0s | Precomputed during SCRFD detection — zero additional cost |
| FaceNet | 0.046s | Separate model, requires face crop + resize to 160x160 |
| ArcFace (DeepFace) | 0.103s | Same ArcFace architecture, heavier wrapper overhead |

No ground truth was available, so verification metrics (AUC, optimal threshold) were not computed. The key finding is that ArcFace via InsightFace is effectively free when using SCRFD.

## Phase 3: Clustering

146 ArcFace embeddings clustered with three algorithms:

| Algorithm | Best Config | Clusters | Noise |
|-----------|-------------|----------|------:|
| Chinese Whispers | threshold=0.5 | 82 | 0 |
| DBSCAN | eps=0.7 | 16 | 56 |
| HDBSCAN | min_cluster_size=2 | 22 | 32 |

- Chinese Whispers produces no noise (every face gets a cluster) but tends to over-cluster at low thresholds.
- DBSCAN and HDBSCAN produce more realistic cluster counts but mark many faces as noise.
- Without ground truth labels, we cannot determine which is most accurate. Visual inspection of the 16-22 cluster range from DBSCAN/HDBSCAN seems reasonable for 43 photos.

## Phase 4: Quality Filters

Distribution of detected face properties (SCRFD, 146 faces):

| Property | Min | Max | Mean | Median |
|----------|----:|----:|-----:|-------:|
| Confidence | 0.511 | 0.921 | 0.808 | 0.842 |
| Face size (px) | 29 | 1998 | 291.9 | 188.0 |
| Blur score | 4.77 | 925.17 | 187.83 | 115.36 |
| Embedding norm | 8.51 | 27.63 | 21.40 | 21.92 |

SCRFD already filters at ~0.5 confidence internally, so the 0.3 and 0.5 thresholds remove nothing. Blur is the most aggressive filter (blur>=200 removes 72% of faces). A reasonable starting point: confidence>=0.5, size>=32px, blur>=50 keeps 103/146 faces (70%).

## Recommendation for Frank Sherlock

**Use InsightFace (buffalo_l model pack) with SCRFD + ArcFace.**

Rationale:
1. **Single library, single pass**: SCRFD detection + ArcFace embedding in one call. No need to manage separate detector and embedding models.
2. **Good detection quality**: 146 faces across 43 images with median confidence 0.84. Not the highest count (MTCNN finds more) but likely fewer false positives.
3. **Free embeddings**: ArcFace 512-dim embeddings are computed as part of detection — zero additional time.
4. **Proven stack**: This is what Immich and other photo managers use. The buffalo_l model pack is well-maintained.
5. **Clustering**: Chinese Whispers (what Immich uses) or HDBSCAN are both viable. Need ground truth testing to pick definitively, but HDBSCAN with min_cluster_size=2 is a safe default.
6. **Quality filter**: Start with blur>=50 as the main noise filter (removes ~29% of low-quality detections).

**Integration concern**: InsightFace is Python/ONNX. For Rust integration, options are:
- Run as a Python subprocess (like Surya OCR) with the ONNX models
- Use `ort` (ONNX Runtime Rust bindings) to load the SCRFD and ArcFace ONNX models directly
- Hybrid: Python for initial model download/setup, Rust ONNX for runtime inference
