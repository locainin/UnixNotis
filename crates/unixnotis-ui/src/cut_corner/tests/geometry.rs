use unixnotis_core::CutCorners;

use gtk::{graphene, gsk};

use super::super::geometry::{build_path, contains_point};

#[test]
fn hit_testing_rejects_clipped_pixels_and_accepts_the_plate() {
    let corners = CutCorners {
        top_left: 20,
        top_right: 20,
        bottom_right: 20,
        bottom_left: 20,
    };

    assert!(!contains_point(100.0, 80.0, corners, 1.0, 1.0));
    assert!(!contains_point(100.0, 80.0, corners, 99.0, 1.0));
    assert!(contains_point(100.0, 80.0, corners, 50.0, 40.0));
    assert!(contains_point(100.0, 80.0, corners, 20.0, 0.0));
}

#[test]
fn oversized_corner_values_are_bounded_to_non_crossing_edges() {
    let corners = CutCorners {
        top_left: u16::MAX,
        top_right: u16::MAX,
        bottom_right: u16::MAX,
        bottom_left: u16::MAX,
    };

    assert!(!contains_point(100.0, 40.0, corners, 1.0, 1.0));
    assert!(contains_point(100.0, 40.0, corners, 50.0, 20.0));
}

#[test]
fn hit_testing_rejects_every_point_outside_the_allocation() {
    let corners = CutCorners::default();

    assert!(!contains_point(100.0, 40.0, corners, -1.0, 20.0));
    assert!(!contains_point(100.0, 40.0, corners, 50.0, -1.0));
    assert!(!contains_point(100.0, 40.0, corners, 100.0, 20.0));
    assert!(!contains_point(100.0, 40.0, corners, 50.0, 40.0));
}

#[test]
fn hit_testing_includes_near_edges_and_cuts_each_corner_independently() {
    let corners = CutCorners {
        top_left: 8,
        top_right: 12,
        bottom_right: 16,
        bottom_left: 20,
    };

    // Each pair straddles one diagonal so all four corner equations stay covered
    for (outside, inside) in [
        ((2.0, 2.0), (4.0, 4.0)),
        ((96.0, 2.0), (94.0, 6.0)),
        ((94.0, 46.0), (90.0, 40.0)),
        ((4.0, 46.0), (12.0, 38.0)),
    ] {
        assert!(!contains_point(100.0, 48.0, corners, outside.0, outside.1));
        assert!(contains_point(100.0, 48.0, corners, inside.0, inside.1));
    }

    assert!(contains_point(100.0, 48.0, CutCorners::default(), 0.0, 0.0));
    assert!(contains_point(
        100.0,
        48.0,
        CutCorners::default(),
        99.999,
        47.999
    ));
}

#[test]
fn rendered_path_and_pointer_shape_match_across_the_plate() {
    let width = 37.0;
    let height = 29.0;
    let corners = CutCorners {
        top_left: 5,
        top_right: 9,
        bottom_right: 12,
        bottom_left: 7,
    };
    let path = build_path(width, height, corners);

    // A dense grid catches drift between the visible polygon and pointer hit testing
    for y in 0_u16..29 {
        for x in 0_u16..37 {
            // Unequal fractions avoid sampling directly on a diagonal boundary
            let x = f32::from(x) + 0.33;
            let y = f32::from(y) + 0.21;
            assert_eq!(
                path.in_fill(&graphene::Point::new(x, y), gsk::FillRule::Winding),
                contains_point(
                    f64::from(width),
                    f64::from(height),
                    corners,
                    f64::from(x),
                    f64::from(y)
                ),
                "path and hit test differ at ({x}, {y})"
            );
        }
    }
}
