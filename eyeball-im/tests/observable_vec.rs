use imbl::vector;
use eyeball_im::{ObservableVector, ObservableVectorTransaction};

#[test]
#[should_panic]
fn capacity_overflow(){
    let ob: ObservableVector<usize> = ObservableVector::with_capacity(usize::MAX / 2);
}

#[test]
#[should_panic]
fn set_out_of_range(){
    let mut ob = ObservableVector::<usize>::new();
    ob.append(vector![10, 20]);
    ob.set(2, 30);
}

#[test]
#[should_panic]
fn transaction_set_out_of_range(){
    let mut inner = ObservableVector::default();
    let mut txn = ObservableVectorTransaction::new(&mut inner);
    txn.set(0, 1);
}

