extern crate json;



#[test]
fn test_parse_decode(){
    let parsed = json::parse(r#"
        { "type": "decode",
          "io_id": 0
          }

        "#).unwrap();

      let instantiated = object!{
            "type" => "decode",
            "io_id" => 0
        };

    assert_eq!(parsed, instantiated);
}