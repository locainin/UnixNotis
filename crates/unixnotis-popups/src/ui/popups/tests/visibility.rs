use super::{visible_popup_restack_ids, visible_popup_target};

#[test]
fn visible_target_stays_within_popup_and_runtime_limits() {
    assert_eq!(visible_popup_target(2, 5), 2);
    assert_eq!(visible_popup_target(5, 2), 2);
    assert_eq!(visible_popup_target(0, 3), 0);
}

#[test]
fn stable_visible_order_requires_no_restack() {
    assert!(visible_popup_restack_ids(&[9, 8, 7], &[9, 8, 7]).is_empty());
}

#[test]
fn prepending_marks_only_the_new_front_row() {
    let moved = visible_popup_restack_ids(&[8, 7], &[9, 8, 7]);

    assert_eq!(moved.len(), 1);
    assert!(moved.contains(&9));
}

#[test]
fn swapping_rows_marks_only_the_row_that_must_move() {
    let moved = visible_popup_restack_ids(&[9, 8, 7], &[8, 9, 7]);

    assert_eq!(moved.len(), 1);
    assert!(moved.contains(&8));
}

#[test]
fn restack_plan_matches_reference_for_every_four_row_permutation() {
    let original = [1, 2, 3, 4];
    for first in original {
        for second in original {
            for third in original {
                for fourth in original {
                    let desired = [first, second, third, fourth];
                    let unique = desired
                        .iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>();
                    if unique.len() != original.len() {
                        continue;
                    }

                    let mut working = original.to_vec();
                    let mut expected = std::collections::HashSet::new();
                    for (target, id) in desired.iter().copied().enumerate() {
                        let current = working
                            .iter()
                            .position(|value| *value == id)
                            .expect("permutation should contain every row");
                        if current != target {
                            let moved = working.remove(current);
                            working.insert(target, moved);
                            expected.insert(id);
                        }
                    }

                    assert_eq!(visible_popup_restack_ids(&original, &desired), expected);
                }
            }
        }
    }
}
