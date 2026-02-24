/// Perceptual similarity scoring and near-duplicate grouping.
///
/// Uses dHash (difference hash) for visual similarity and Jaccard word
/// overlap for description similarity, combined with configurable weights.

/// Hamming distance between two 64-bit hashes (number of differing bits).
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Visual similarity from two dHash values: `1.0 - dist/64.0`.
pub fn dhash_similarity(a: u64, b: u64) -> f32 {
    1.0 - hamming_distance(a, b) as f32 / 64.0
}

/// Jaccard word overlap between two descriptions.
///
/// Words are lowercased and filtered to length > 1.
pub fn description_similarity(a: &str, b: &str) -> f32 {
    use std::collections::HashSet;
    let words = |s: &str| -> HashSet<String> {
        s.split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() > 1)
            .collect()
    };
    let set_a = words(a);
    let set_b = words(b);
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f32 / union as f32
}

/// Combined similarity: 85% visual (dHash) + 15% textual (description).
pub fn combined_similarity(dhash_a: u64, dhash_b: u64, desc_a: &str, desc_b: &str) -> f32 {
    let visual = dhash_similarity(dhash_a, dhash_b);
    let textual = description_similarity(desc_a, desc_b);
    0.85 * visual + 0.15 * textual
}

/// Candidate file for near-duplicate grouping.
pub struct NearDupCandidate {
    pub dhash: u64,
    pub description: String,
    pub fingerprint: String,
}

/// Group near-duplicate candidates using Union-Find.
///
/// Returns groups of indices into the `candidates` slice where each pair
/// exceeds `threshold`. Skips pairs that share the same fingerprint (those
/// are already handled by exact-duplicate detection). Groups with only one
/// member are excluded from the output.
pub fn group_near_duplicates(candidates: &[NearDupCandidate], threshold: f32) -> Vec<Vec<usize>> {
    let n = candidates.len();
    if n < 2 {
        return Vec::new();
    }

    // Max Hamming distance to even consider a pair (early exit optimization).
    // If visual alone can't reach the threshold, skip the pair entirely.
    // visual_min = (threshold - 0.15) / 0.85  (worst case: textual = 1.0)
    // hamming_max = (1.0 - visual_min) * 64
    let visual_min = ((threshold - 0.15) / 0.85).max(0.0);
    let max_hamming = ((1.0 - visual_min) * 64.0).floor() as u32;

    let mut parent: Vec<usize> = (0..n).collect();
    let mut rank: Vec<usize> = vec![0; n];

    fn find(parent: &mut [usize], i: usize) -> usize {
        let mut root = i;
        while parent[root] != root {
            root = parent[root];
        }
        // Path compression
        let mut current = i;
        while parent[current] != root {
            let next = parent[current];
            parent[current] = root;
            current = next;
        }
        root
    }

    fn union(parent: &mut [usize], rank: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra == rb {
            return;
        }
        if rank[ra] < rank[rb] {
            parent[ra] = rb;
        } else if rank[ra] > rank[rb] {
            parent[rb] = ra;
        } else {
            parent[rb] = ra;
            rank[ra] += 1;
        }
    }

    for i in 0..n {
        for j in (i + 1)..n {
            // Skip pairs with identical fingerprint (exact duplicates)
            if candidates[i].fingerprint == candidates[j].fingerprint {
                continue;
            }
            // Early exit: if Hamming distance is too large, skip
            let dist = hamming_distance(candidates[i].dhash, candidates[j].dhash);
            if dist > max_hamming {
                continue;
            }
            let sim = combined_similarity(
                candidates[i].dhash,
                candidates[j].dhash,
                &candidates[i].description,
                &candidates[j].description,
            );
            if sim >= threshold {
                union(&mut parent, &mut rank, i, j);
            }
        }
    }

    // Collect groups
    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }

    groups.into_values().filter(|g| g.len() > 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamming_identical() {
        assert_eq!(hamming_distance(0, 0), 0);
        assert_eq!(hamming_distance(u64::MAX, u64::MAX), 0);
    }

    #[test]
    fn hamming_one_bit() {
        assert_eq!(hamming_distance(0, 1), 1);
        assert_eq!(hamming_distance(0b1010, 0b1000), 1);
    }

    #[test]
    fn hamming_all_different() {
        assert_eq!(hamming_distance(0, u64::MAX), 64);
    }

    #[test]
    fn dhash_sim_identical() {
        let sim = dhash_similarity(0xABCD, 0xABCD);
        assert!((sim - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn dhash_sim_opposite() {
        let sim = dhash_similarity(0, u64::MAX);
        assert!(sim.abs() < f32::EPSILON);
    }

    #[test]
    fn dhash_sim_close() {
        // 4 bits differ out of 64 → similarity = 1.0 - 4/64 = 0.9375
        let a: u64 = 0;
        let b: u64 = 0b1111;
        let sim = dhash_similarity(a, b);
        assert!((sim - 0.9375).abs() < 0.001);
    }

    #[test]
    fn desc_sim_identical() {
        let sim = description_similarity("a sunny beach photo", "a sunny beach photo");
        assert!((sim - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn desc_sim_no_overlap() {
        let sim = description_similarity("cat sleeping indoor", "mountain river outdoor");
        assert!(sim < 0.01);
    }

    #[test]
    fn desc_sim_partial_overlap() {
        let sim = description_similarity("sunny beach photo", "sunny mountain photo");
        // words: {sunny, beach, photo} vs {sunny, mountain, photo}
        // intersection: {sunny, photo} = 2, union: {sunny, beach, photo, mountain} = 4
        // Jaccard = 2/4 = 0.5
        assert!((sim - 0.5).abs() < 0.01);
    }

    #[test]
    fn desc_sim_both_empty() {
        assert!((description_similarity("", "") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn desc_sim_filters_short_words() {
        // "a" has length 1, filtered out
        let sim = description_similarity("a cat", "a dog");
        // Only "cat" vs "dog" → 0 intersection, 2 union → 0.0
        assert!(sim.abs() < f32::EPSILON);
    }

    #[test]
    fn combined_sim_identical() {
        let sim = combined_similarity(0xABCD, 0xABCD, "beach photo", "beach photo");
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn combined_sim_weights() {
        // visual = 1.0 (same hash), text = 0.0 (no overlap)
        let sim = combined_similarity(0, 0, "cat", "dog");
        // 0.85 * 1.0 + 0.15 * 0.0 = 0.85
        assert!((sim - 0.85).abs() < 0.01);
    }

    #[test]
    fn group_empty_input() {
        let groups = group_near_duplicates(&[], 0.85);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_single_input() {
        let candidates = vec![NearDupCandidate {
            dhash: 0,
            description: "test".into(),
            fingerprint: "fp1".into(),
        }];
        let groups = group_near_duplicates(&candidates, 0.85);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_similar_pair() {
        let candidates = vec![
            NearDupCandidate {
                dhash: 0,
                description: "sunny beach".into(),
                fingerprint: "fp1".into(),
            },
            NearDupCandidate {
                dhash: 0b11, // 2 bits differ → visual sim = 1 - 2/64 ≈ 0.97
                description: "sunny beach".into(),
                fingerprint: "fp2".into(),
            },
        ];
        let groups = group_near_duplicates(&candidates, 0.85);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
    }

    #[test]
    fn group_skips_same_fingerprint() {
        let candidates = vec![
            NearDupCandidate {
                dhash: 0,
                description: "test".into(),
                fingerprint: "fp-same".into(),
            },
            NearDupCandidate {
                dhash: 0, // identical hash but same fingerprint
                description: "test".into(),
                fingerprint: "fp-same".into(),
            },
        ];
        let groups = group_near_duplicates(&candidates, 0.85);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_dissimilar_not_grouped() {
        let candidates = vec![
            NearDupCandidate {
                dhash: 0,
                description: "cat".into(),
                fingerprint: "fp1".into(),
            },
            NearDupCandidate {
                dhash: u64::MAX, // max distance
                description: "dog".into(),
                fingerprint: "fp2".into(),
            },
        ];
        let groups = group_near_duplicates(&candidates, 0.85);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_transitive_chaining() {
        // A ~ B and B ~ C should put A, B, C in the same group
        let candidates = vec![
            NearDupCandidate {
                dhash: 0,
                description: "beach sunset".into(),
                fingerprint: "fp1".into(),
            },
            NearDupCandidate {
                dhash: 0b11, // close to A
                description: "beach sunset".into(),
                fingerprint: "fp2".into(),
            },
            NearDupCandidate {
                dhash: 0b1100, // close to B (4 bits from A, 4 from B but combined still above threshold)
                description: "beach sunset".into(),
                fingerprint: "fp3".into(),
            },
        ];
        let groups = group_near_duplicates(&candidates, 0.85);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 3);
    }
}
