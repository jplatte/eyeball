use eyeball_im::{ObservableVector, ObservableVectorTransaction};
use imbl::vector;

#[test]
#[should_panic]
fn zero_capacity() {
    let _ob: ObservableVector<i32> = ObservableVector::with_capacity(0);
}

#[test]
#[should_panic]
fn capacity_overflow() {
    let _ob: ObservableVector<usize> = ObservableVector::with_capacity(usize::MAX / 2);
}

#[test]
#[should_panic]
fn insert_out_of_range() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    ob.insert(1, -1);
}

#[test]
#[should_panic]
fn set_out_of_range() {
    let mut ob = ObservableVector::<usize>::new();
    ob.append(vector![10, 20]);
    ob.set(2, 30);
}

#[test]
#[should_panic]
fn remove_out_of_range() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    ob.remove(0);
}

#[test]
#[should_panic]
fn transaction_insert_out_of_range() {
    let mut ob = ObservableVector::new();
    let mut txn = ob.transaction();
    txn.insert(1, 1);
}

#[test]
#[should_panic]
fn transaction_set_out_of_range() {
    let mut ob = ObservableVector::new();
    let mut txn = ob.transaction();
    txn.set(0, 1);
}

#[test]
#[should_panic]
fn transaction_remove_out_of_range() {
    let mut ob: ObservableVector<i32> = ObservableVector::new();
    let mut txn = ob.transaction();
    txn.remove(0);
}

#[test]
#[should_panic]
fn transaction_entry_out_of_range() {
    let mut ob = ObservableVector::new();
    ob.append(vector![1]);
    let mut txn = ob.transaction();
    txn.entry(1);
}
