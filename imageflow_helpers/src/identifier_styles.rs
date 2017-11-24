use regex::{Regex, Captures};

// TODO: Document handling of whitespace within an identifier, and what identifiers this will work on



#[derive(Copy,Clone,PartialEq,Debug)]
pub enum Transform{
    AddUnderscores,
    Capitalize,
    LowerFirst,
    StripUnderscores
}
#[derive(Copy,Clone,PartialEq,Debug)]
pub enum Style{
    Snake,
    ScreamingSnake,
    PascalCase,
    PascalSnake,
    CamelSnake,
    CamelCase
}


pub fn transform(s: &str, transform: Transform) -> String {
    match transform {
        Transform::AddUnderscores => {
            let temp = Regex::new("(?P<lookbehind>[^xy])([0-9]+)").unwrap().replace_all(s, "${lookbehind}_$2");
            let temp = Regex::new("[A-Z]").unwrap().replace_all(&temp, "_$0");
            let temp = Regex::new(r"(\A|\s+)_+").unwrap().replace_all(&temp, "$1");
            let temp = Regex::new(r"_+(\z|\s+)").unwrap().replace_all(&temp, "$1");
            temp.replace("__","_")
        },
        Transform::StripUnderscores => {
            s.replace("_","")
        },
        Transform::Capitalize => {
            Regex::new(r"(_|\b)([a-z])").unwrap().replace_all(s, |c: &Captures | c[0].to_uppercase()).into_owned()
        }
        Transform::LowerFirst => {
            Regex::new(r"(\A|\s+)([A-Z])").unwrap().replace_all(s, |c: &Captures | c[0].to_lowercase()).into_owned()
        }
    }
}

///
/// If the input has any underscores, they must all be in the right places - we'll ignore case
///
pub fn style_id(s: &str, style: Style) -> String{
    let mut temp = s.to_owned();
    //Normalize to underscores (unless there are already some)
    if !temp.contains('_') {
        temp = transform(&temp, Transform::AddUnderscores);
    }
    //Normalize to lower (relying on underscores now)
    let temp = temp.to_lowercase();

    let temp: String = match style{
        Style::PascalSnake | Style::PascalCase  => {
            transform(&temp, Transform::Capitalize)
        },
        Style::CamelCase | Style::CamelSnake => {
            let  t = transform(&temp, Transform::Capitalize);
            transform(&t, Transform::LowerFirst)
        }
        Style::ScreamingSnake => {
            temp.to_uppercase()
        }
        _ => temp
    };

    match style{
        Style::PascalCase | Style::CamelCase => {
            transform(&temp, Transform::StripUnderscores)
        }
        _ => temp
    }
}


#[test]
fn test_styling(){

    assert_eq!(Regex::new("(.)").unwrap().replace_all("a", "${1}_"), "a_" );
    assert_eq!(Regex::new("(?P<char>.)").unwrap().replace_all("a", "${char}_"), "a_" ); //actual: ""

    assert_eq!("Aok_B_C", transform("aok_b_c", Transform::Capitalize));
    assert_eq!("aokbc", transform("aok_b_c", Transform::StripUnderscores));
    assert_eq!("hI hELLO", transform("HI HELLO", Transform::LowerFirst));

    assert_eq!("a_Imageflow_A_B", transform("aImageflowAB", Transform::AddUnderscores));
    assert_eq!(" a_b c", transform("_ a__b __c__", Transform::AddUnderscores));
    assert_eq!("a_102", transform("a102", Transform::AddUnderscores));


    assert_eq!("imageflow_a_b_2d_40", style_id("ImageflowAB2d40", Style::Snake));
    assert_eq!("ImageflowAB2d40", style_id("ImageflowAB2d40", Style::PascalCase));
    assert_eq!("imageflowAB2d40", style_id("ImageflowAB2d40", Style::CamelCase));
    assert_eq!("IMAGEFLOW_A_B_2D_40", style_id("ImageflowAB2d40", Style::ScreamingSnake));
    assert_eq!("imageflow_A_B_2d_40", style_id("ImageflowAB2d40", Style::CamelSnake));
    assert_eq!("Imageflow_A_B_2d_40", style_id("ImageflowAB2d40", Style::PascalSnake));

    assert_eq!("struct a_Imageflow_A_B_3so_10_A_2", transform("struct aImageflowAB3so10A2", Transform::AddUnderscores));

}

