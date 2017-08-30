
extern crate cheddar;
extern crate regex;
extern crate imageflow_helpers;
use imageflow_helpers::identifier_styles::*;

use regex::{Regex, Captures};
use std::io::Write;
use std::path;
use cheddar::*;

//We've reimplemented I/O for cheddar so we can post-process

pub fn write<F, P: AsRef<path::Path>>(cheddar: &Cheddar, file: P, filter:F) -> Result<(), Vec<Error>>where
    F:  FnOnce(String) -> String  {
    let file = file.as_ref();

    if let Some(dir) = file.parent() {
        if let Err(error) = std::fs::create_dir_all(dir) {
            panic!("could not create directories in '{}': {}", dir.display(), error);
        }
    }

    let file_name = file.file_stem().map_or("default".into(), |os| os.to_string_lossy());
    let header = cheddar.compile(&file_name)?;
    let filtered_header = filter(header);

    let bytes_buf = filtered_header.into_bytes();
    if let Err(error) = std::fs::File::create(&file).and_then(|mut f| f.write_all(&bytes_buf)) {
        panic!("could not write to '{}': {}", file.display(), error);
    } else {
        Ok(())
    }
}

pub fn run_build<F, P: AsRef<path::Path>>(cheddar: &Cheddar,  file: P, filter: F) where
F:  FnOnce(String) -> String {
    if let Err(errors) = write(cheddar, file, filter) {
        for error in &errors {
            cheddar.print_error(error);
        }

        panic!("errors compiling header file");
    }
}

static OPAQUE_STRUCTS: &'static str = r#"
struct Context;
struct JsonResponse;
struct Job;
struct JobIo;
        "#;

static DEFINE_INTS: &'static str = r#"
typedef signed byte int8_t;
typedef signed long int64_t;
typedef signed int int32_t;
typedef unsigned byte uint8_t;
        "#;

//not used
//static TYPEDEF_VOID_STRUCTS: &'static str = r#"
//typedef void* Context;
//typedef void* JobIo;
//typedef void* Job;
//typedef void* JsonResponse;
//        "#;


include!("src/abi_version.rs");


fn get_version_consts() -> String{

    format!("\n// Incremented for breaking changes\n#define IMAGEFLOW_ABI_VER_MAJOR {}\n\n// Incremented for non-breaking additions\n#define IMAGEFLOW_ABI_VER_MINOR {}\n\n", IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR)
}



fn rename_word_excluding_enum_members(input: String, old_name: &str, new_name_before_casing: &str, change_case: Style) -> String{
    let find_str = r"\b".to_owned() + old_name + r"\b(\s*)(.)";
    let new_name = style_id(new_name_before_casing, change_case);
    let s = Regex::new(&find_str).unwrap().replace_all(&input, |caps: &Captures| {
        if &caps[2] == "="  {
            caps[0].to_owned() //This is an enum member, skip
        }else{
            format!("{}{}{}", new_name, &caps[1], &caps[2])
        }
    });
    s.into_owned()
}

fn rename_enum_snake_case_and_prefix_members(input: String, old_name: &str, new_name: String, change_case: Style, member_prefix: &str, member_casing: Style) -> String {

    let new_name = style_id(&new_name, change_case);

    let new_ref = &new_name;

    let rename_term = format!("\\b{}\\b", old_name);
    let s = input;
    let s = Regex::new(&rename_term).unwrap().replace_all(&s, |_: &Captures| new_ref.to_owned() );

    let find_def_str = r"\btypedef\s+enum\s+".to_owned() + &new_name + r"\s+(\{[^\}]+\})";

    let moz_cheddar_prefix = format!("{}_", old_name);

    let s = Regex::new(&find_def_str).unwrap().replace(&s, |outer_caps: &Captures| {

        let re_member = Regex::new(r"\b([A-Za-z0-9_]+)\s+=").unwrap();

        let contents = re_member.replace_all(&outer_caps[1], | caps: &Captures| {
            let without_moz_cheddar_prefix = caps[1].replace(&moz_cheddar_prefix,"");
            let snake_id = style_id(&without_moz_cheddar_prefix, Style::Snake);
            let full_snake_id = if member_prefix == "" {
                snake_id
            }else {
               format!("{}_{}", style_id(member_prefix, Style::Snake), snake_id)
            };
            format!("{} =", style_id(&full_snake_id, member_casing))
        });
        format!("typedef enum {} {}", new_ref, contents)
    });
    s.into()
}

#[derive(Copy,Clone,PartialEq,Debug)]
enum StructModification{
    //Replace with void
    Erase,
    //You can specify no prefix..
    Prefix {
        prefix: &'static str,
        style: Style
    }
}


fn filter_structs(s: String, names: &[&str], how: StructModification) -> String{
    let mut temp = s;
    temp = match  how{
        StructModification::Erase => {
            for n in names {
                temp = rename_word_excluding_enum_members(temp, n, "void", Style::Snake);
            }
            temp
        }
        StructModification::Prefix{ prefix, style} => {
            for n in names {
                let new_name = format!("struct {}{}", prefix, n);
                temp = rename_word_excluding_enum_members(temp, n, &new_name, style);
            }
            temp
        }
    };

    //De-duplicate and lowercase struct
    temp = Regex::new(r"(?i)\bstruct\b(\s+struct)*").unwrap().replace_all(&temp, "struct").into();
    //Drop our opaque structs decl.
    temp = Regex::new(r"\bstruct void;").unwrap().replace_all(&temp, "").into();
    temp
}

#[derive(Copy,Clone,PartialEq,Debug)]
struct EnumModification{
    name_prefix: &'static str,
    member_prefix: &'static str,
    name_style: Style,
    member_style: Style
}


fn filter_enums<'a,'b>(s: String,  names: &'a[&'a str],  how: EnumModification) -> String{
    let mut temp =s;
    for n in names{
        let new_name = format!("{}{}", how.name_prefix, n);
        let member_prefix = if how.member_prefix == ""{
            "".to_owned()
        }else{
            new_name.to_owned()
        };
        temp = rename_enum_snake_case_and_prefix_members(temp, n, new_name, how.name_style, &member_prefix, how.member_style);
    }
    temp
}

static ENUM_NAMES: [&'static str; 4] = ["IoMode", "Direction", "Lifetime", "CleanupWith"];
static STRUCT_NAMES: [&'static str; 4] = ["Job", "JobIo", "Context", "JsonResponse"];



#[derive(Copy,Clone,PartialEq,Debug)]
enum Target{
    Raw,
    PInvoke,
    Default,
    Lua,
    SignaturesOnly,
    PrefixAll{ prefix: &'static str, struct_name: Style, enum_name: Style, enum_member: Style},
    Other{structs: StructModification, enums: EnumModification}
}

fn strip_preprocessor_directives(contents: &str) -> String{
    //Strip the extern C stuff
    let temp = Regex::new(r"(?im)^\s*\#\s*ifdef\s+__cplusplus[^\#]+\#\s*endif").unwrap().replace_all(&contents, "");
    //Strip all ifndef/ifdef statements
    //let temp2 = Regex::new(r"(?im)^\s*\#\s*(ifdef|ifndef|endif).*").unwrap().replace_all(&temp, "");
    //Strip ALL # preprocessor directives
    let temp2 = Regex::new(r"(?im)^\s*\#\s*.*").unwrap().replace_all(&temp, "").into();

    temp2
}


fn strip_comments(contents: &str) -> String{
    let temp = Regex::new(r"//[^\n\r]*").unwrap().replace_all(&contents, "");
    let temp2 = Regex::new(r"[\n\r\s\t]+\n").unwrap().replace_all(&temp, "\n\n").into();
    temp2
}
fn build(file: String, target: Target){

    let insert = match target{
        Target::PInvoke => format!("{}\n\n{}",get_version_consts(), DEFINE_INTS),
        _ =>  format!("{}\n\n{}",get_version_consts(), OPAQUE_STRUCTS)
    };

    if target == Target::Raw {
        run_build(cheddar::Cheddar::new().expect("could not read manifest")
                      .insert_code(&insert), file, |s| s);
    }else {
        let should_strip_preprocessor_directives = target == Target::Lua || target == Target::SignaturesOnly;
        let should_strip_comments = target == Target::SignaturesOnly;
        let target = match target {
            Target::PrefixAll { prefix, struct_name, enum_name, enum_member } => Target::Other {
                structs: StructModification::Prefix { prefix: prefix, style: struct_name },
                enums: EnumModification {
                    name_prefix: prefix, name_style: enum_name,
                    member_prefix: prefix, member_style: enum_member
                }
            },
            Target::Default | Target::Lua => Target::Other {
                structs: StructModification::Prefix { prefix: "Imageflow", style: Style::Snake },
                enums: EnumModification {
                    name_prefix: "Imageflow", name_style: Style::Snake,
                    member_prefix: "Imageflow", member_style: Style::Snake
                }
            },
            Target::PInvoke => Target::Other {
                structs: StructModification::Erase,
                enums: EnumModification {
                    name_prefix: "", name_style: Style::PascalCase,
                    member_prefix: "", member_style: Style::PascalCase
                }
            },
             Target::SignaturesOnly => Target::Other {
                structs: StructModification::Erase,
                enums: EnumModification {
                    name_prefix: "Imageflow", name_style: Style::Snake,
                    member_prefix: "Imageflow", member_style: Style::Snake
                }
            },
            t => t
        };
        if let Target::Other { structs, enums } = target {
            run_build(cheddar::Cheddar::new().expect("could not read manifest")
                          .insert_code(&insert), file, |s: String| -> String {
                let temp = filter_enums(filter_structs(s, &STRUCT_NAMES, structs), &ENUM_NAMES, enums);
                let temp = if should_strip_preprocessor_directives{
                    strip_preprocessor_directives(&temp)
                }else {
                    temp
                };
                if should_strip_comments{
                    strip_comments(&temp)
                } else{
                    temp
                }
            });
        } else {
            panic!("");
        }
    }
}

fn main() {
    //let base = "imageflow_"; //for debugging more easily
    let base = "../bindings/headers/imageflow_";

    build(format!("{}default.h",base), Target::Default);

    build(format!("{}lua.h",base), Target::Lua);
    build(format!("{}raw.h",base), Target::Raw);

    build(format!("{}short.h",base), Target::SignaturesOnly);
    build(format!("{}pinvoke.h",base), Target::PInvoke);

    build(format!("{}SCREAMING_SNAKE.h",base), Target::PrefixAll{
        prefix: "Imageflow",
        struct_name: Style::ScreamingSnake,
        enum_name: Style::ScreamingSnake,
        enum_member: Style::ScreamingSnake,
    });
    build(format!("{}SCREAMING_ENUMS.h",base), Target::PrefixAll{
        prefix: "Imageflow",
        struct_name: Style::Snake,
        enum_name: Style::ScreamingSnake,
        enum_member: Style::ScreamingSnake,
    });
    build(format!("{}PascalCase.h",base), Target::PrefixAll{
        prefix: "",
        struct_name: Style::PascalCase,
        enum_name: Style::PascalCase,
        enum_member: Style::PascalCase,
    });
    build(format!("{}PrefixedPascalCase.h",base), Target::PrefixAll{
        prefix: "Imageflow",
        struct_name: Style::PascalCase,
        enum_name: Style::PascalCase,
        enum_member: Style::PascalCase,
    });
    build(format!("{}Prefixed_Pascal_Snake.h",base), Target::PrefixAll{
        prefix: "Imageflow",
        struct_name: Style::PascalSnake,
        enum_name: Style::PascalSnake,
        enum_member: Style::PascalSnake,
    });

    build(format!("{}prefixed_Camel_Snake.h",base), Target::PrefixAll{
        prefix: "Imageflow",
        struct_name: Style::CamelSnake,
        enum_name: Style::CamelSnake,
        enum_member: Style::CamelSnake,
    });
    build(format!("{}prefixedCamelCase.h",base), Target::PrefixAll{
        prefix: "Imageflow",
        struct_name: Style::CamelCase,
        enum_name: Style::CamelCase,
        enum_member: Style::CamelCase,
    });

}
