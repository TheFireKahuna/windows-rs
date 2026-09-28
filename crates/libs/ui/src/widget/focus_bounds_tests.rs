use super::*;

#[test]
fn focus_outline_preserves_clearance_until_it_reaches_the_client_edge() {
    let viewport = Some(Vector2::new(1280.0, 900.0));
    let size = Vector2::new(22.0, 28.0);
    for (at, expected_at, expected_size) in [
        ((10.0, 20.0), (10.0, 20.0), (22.0, 28.0)),
        ((-3.0, 20.0), (0.0, 20.0), (19.0, 28.0)),
        ((1263.0, 20.0), (1263.0, 20.0), (17.0, 28.0)),
        ((10.0, -3.0), (10.0, 0.0), (22.0, 25.0)),
        ((10.0, 875.0), (10.0, 875.0), (22.0, 25.0)),
    ] {
        assert_eq!(focus_box(Vector2::new(at.0, at.1), size, viewport),
            Some((Vector2::new(expected_at.0, expected_at.1),
                Vector2::new(expected_size.0, expected_size.1))));
    }
    for at in [(-30.0, 20.0), (1280.0, 20.0), (10.0, -30.0), (10.0, 900.0)] {
        assert_eq!(focus_box(Vector2::new(at.0, at.1), size, viewport), None);
    }
    assert_eq!(focus_box(Vector2::zero(), size, Some(Vector2::zero())), None);
}
