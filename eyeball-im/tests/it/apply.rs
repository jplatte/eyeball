use imbl::vector;

use eyeball_im::{VectorDiff};

#[test]
fn reset_larger() {
    let mut vec = vector![1, 2, 3];
    VectorDiff::Reset { values: vector![4, 5, 6, 7] }.apply(&mut vec);
    assert_eq!(vec, vector![4, 5, 6, 7]);
}

#[test]
fn reset_same_size() {
    let mut vec = vector![1, 2, 3];
    VectorDiff::Reset { values: vector![4, 5, 6] }.apply(&mut vec);
    assert_eq!(vec, vector![4, 5, 6]);
}

#[test]
fn reset_smaller() {
    let mut vec = vector![1, 2, 3];
    VectorDiff::Reset { values: vector![4, 5] }.apply(&mut vec);
    assert_eq!(vec, vector![4, 5]);
}

#[test]
fn reset_clear() {
    let mut vec = vector![1, 2, 3];
    VectorDiff::Reset { values: vector![] }.apply(&mut vec);
    assert_eq!(vec, vector![]);
}
