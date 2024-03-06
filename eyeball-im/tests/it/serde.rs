use eyeball_im::VectorDiff;
use imbl::vector;

macro_rules! test {
    ($test_name:ident: $vector_diff:expr => $json:expr) => {
        #[test]
        fn $test_name() -> Result<(), Box<dyn std::error::Error>> {
            let vector_diff: VectorDiff<char> = $vector_diff;
            let json = serde_json::to_string(&vector_diff)?;

            assert_eq!(json, $json);

            Ok(())
        }
    };
}

test!(append: VectorDiff::Append { values: vector!['a', 'b'] } => r#"{"Append":{"values":["a","b"]}}"#);
test!(clear: VectorDiff::Clear => r#"{"Clear":{}}"#);
test!(push_front: VectorDiff::PushFront { value: 'a' } => r#"{"PushFront":{"value":"a"}}"#);
test!(push_back: VectorDiff::PushBack { value: 'a' } => r#"{"PushBack":{"value":"a"}}"#);
test!(pop_front: VectorDiff::PopFront => r#"{"PopFront":{}}"#);
test!(pop_back: VectorDiff::PopBack => r#"{"PopBack":{}}"#);
test!(insert: VectorDiff::Insert { index: 42, value: 'a' } => r#"{"Insert":{"index":42,"value":"a"}}"#);
test!(set: VectorDiff::Set { index: 42, value: 'a' } => r#"{"Set":{"index":42,"value":"a"}}"#);
test!(remove: VectorDiff::Remove { index: 42 } => r#"{"Remove":{"index":42}}"#);
test!(truncate: VectorDiff::Truncate { length: 3 } => r#"{"Truncate":{"length":3}}"#);
test!(reset: VectorDiff::Reset { values: vector!['a', 'b'] } => r#"{"Reset":{"values":["a","b"]}}"#);
