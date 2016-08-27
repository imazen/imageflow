#![cfg_attr(feature = "serde_macros", feature(plugin, custom_derive))]
#![cfg_attr(feature = "serde_macros", plugin(serde_macros))]

extern crate serde;
extern crate serde_json;

#[cfg(feature = "serde_macros")]
include!("serde_types.in.rs");

#[cfg(feature = "serde_codegen")]
include!(concat!(env!("OUT_DIR"), "/serde_types.rs"));

#[test]
fn test_roundtrip() {
    let point = Point { x: 1, y: 2 };

    let serialized = serde_json::to_string(&point).unwrap();
    assert_eq!(serialized, r#"{"x":1,"y":2}"#);

    let deserialized: Point = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized,  Point { x: 1, y: 2 });
}


#[test]
fn test_decode_node(){
    let text = r#"{"Decode": { "io_id": 1 } }"#;

    let obj : nodes::AnyNode = serde_json::from_str(&text).unwrap();

    assert_eq!(obj, nodes::AnyNode::Decode(nodes::Decode{ io_id: 1 }));
}


#[test]
fn test_decode_mnode(){
    let text = r#"[{"Decode": { "io_id": 1 } }, {"Encode": { "io_id": 2 } }]"#;

    let obj : Vec<MNode> = serde_json::from_str(&text).unwrap();

    assert_eq!(obj, vec![MNode::Decode{ io_id: 1 }, MNode::Encode{ io_id: 2, encoder: None }]);
}

macro_rules! hashmap {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = ::std::collections::HashMap::new();
         $( map.insert($key, $val); )*
         map
    }}
}

#[test]
fn test_decode_graph(){
    let text = r#"{
        "nodes": {
            "0": {"Decode": { "io_id": 1 } },
            "1": {"Rotate90" : null}

        },
        "edges": [
            {"from": 0, "to": 1, "kind": "Input"}
        ]
    }"#;

    let obj : Graph = serde_json::from_str(&text).unwrap();
    let expected = Graph{
        nodes: hashmap![ 0 => Node::Decode{ io_id: 1 },
                         1 => Node::Rotate90
        ],
        edges: vec![
            Edge{from: 0, to: 1, kind: EdgeKind::Input}
        ]
    };

    assert_eq!(obj, expected);
}

