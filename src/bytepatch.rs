use crate::patches::Patch;

#[derive(Debug, PartialEq, Eq)]
pub enum PatchOutcome {
    Applied(usize),
    AlreadyApplied,
    NotFound,
    Ambiguous(usize),
}

pub fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }

    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect()
}

pub fn apply_patch(buffer: &mut [u8], patch: &Patch) -> PatchOutcome {
    debug_assert_eq!(patch.original.len(), patch.replacement.len());

    let original_matches = find_all(buffer, patch.original);
    let replacement_matches = find_all(buffer, patch.replacement);

    match (original_matches.len(), replacement_matches.len()) {
        (1, 0) => {
            let start = original_matches[0];
            let changed_index = patch
                .original
                .iter()
                .zip(patch.replacement)
                .position(|(original, replacement)| original != replacement)
                .unwrap_or(0);

            buffer[start..start + patch.replacement.len()].copy_from_slice(patch.replacement);
            PatchOutcome::Applied(start + changed_index)
        }
        (0, 1) => PatchOutcome::AlreadyApplied,
        (0, 0) => PatchOutcome::NotFound,
        (count, _) => PatchOutcome::Ambiguous(count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_image() -> Vec<u8> {
        let mut image = vec![0u8; 96];
        image[8..8 + 7].copy_from_slice(crate::patches::PATCHES[0].original);
        image[48..48 + 13].copy_from_slice(crate::patches::PATCHES[1].original);
        image
    }

    #[test]
    fn applies_both_patches() {
        let mut image = synthetic_image();

        assert_eq!(
            apply_patch(&mut image, &crate::patches::PATCHES[0]),
            PatchOutcome::Applied(8)
        );
        assert_eq!(
            apply_patch(&mut image, &crate::patches::PATCHES[1]),
            PatchOutcome::Applied(52)
        );
        assert_eq!(&image[8..8 + 7], crate::patches::PATCHES[0].replacement);
        assert_eq!(&image[48..48 + 13], crate::patches::PATCHES[1].replacement);
    }

    #[test]
    fn detects_already_applied_patches() {
        let mut image = synthetic_image();
        apply_patch(&mut image, &crate::patches::PATCHES[0]);
        apply_patch(&mut image, &crate::patches::PATCHES[1]);

        assert_eq!(
            apply_patch(&mut image, &crate::patches::PATCHES[0]),
            PatchOutcome::AlreadyApplied
        );
        assert_eq!(
            apply_patch(&mut image, &crate::patches::PATCHES[1]),
            PatchOutcome::AlreadyApplied
        );
    }

    #[test]
    fn reports_missing_signature() {
        let mut image = vec![0u8; 64];
        assert_eq!(
            apply_patch(&mut image, &crate::patches::PATCHES[1]),
            PatchOutcome::NotFound
        );
    }

    #[test]
    fn rejects_ambiguous_signature() {
        let mut image = vec![0u8; 128];
        image[0..13].copy_from_slice(crate::patches::PATCHES[1].original);
        image[64..77].copy_from_slice(crate::patches::PATCHES[1].original);

        assert_eq!(
            apply_patch(&mut image, &crate::patches::PATCHES[1]),
            PatchOutcome::Ambiguous(2)
        );
    }
}
