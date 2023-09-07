use imbl::vector;

use eyeball_im::{ObservableVector, ObservableVectorEntry};

#[test]
fn entry() {
    let mut ob: ObservableVector<u8> = ObservableVector::from(vector![1, 2]);
    ObservableVectorEntry::set(&mut ob.entry(1), 3);
    ObservableVectorEntry::remove(ob.entry(0));

    assert_eq!(ob.into_inner(), vector![3]);
}

#[test]
#[should_panic]
fn entry_out_of_range() {
    let mut ob: ObservableVector<String> = ObservableVector::new();
    ob.entry(0);
}

#[test]
fn entries() {
    let mut ob = ObservableVector::from(vector![1, 2, 3]);
    let mut entries = ob.entries();
    while let Some(mut entry) = entries.next() {
        if ObservableVectorEntry::index(&entry) == 1 {
            break;
        }

        ObservableVectorEntry::set(&mut entry, 5);
    }

    assert_eq!(ob.into_inner(), vector![5, 2, 3]);
}

#[test]
fn remove_entries() {
    let mut ob = ObservableVector::from(vector![1, 2, 3]);
    let mut entries = ob.entries();
    while let Some(entry) = entries.next() {
        if ObservableVectorEntry::index(&entry) == 1 {
            unreachable!("index stays 0 is we remove all elements");
        }

        ObservableVectorEntry::remove(entry);
    }
}
