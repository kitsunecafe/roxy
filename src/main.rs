use std::{
    collections::HashMap,
    fmt::Display,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
};

use chrono::serde::ts_seconds_option;
use chrono::{DateTime, Utc};
use glob::glob;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = match Tera::new("examples/basic/templates/**/*") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        };
        tera.autoescape_on(vec![]);
        tera
    };
}

#[derive(Serialize, Deserialize)]
struct Content {
    pub path: String,
    pub title: String,
    #[serde(with = "ts_seconds_option")]
    pub date: Option<DateTime<Utc>>,
    pub content: String,
}

fn create_files(content: Vec<Content>, base_context: &Context) -> io::Result<()> {
    Ok(())
}


fn compile_content_map<'a>(contents: &'a Vec<Content>) -> HashMap<String, Vec<&'a Content>> {
    let mut hm: HashMap<String, Vec<&'a Content>> = HashMap::new();
    let mut default = Vec::new();

    for content in contents.iter() {
        if let Some((section, _)) = content.path.split_once(std::path::MAIN_SEPARATOR_STR) {
            if let Some(vec) = hm.get_mut(section) {
                vec.push(content);
            } else {
                hm.insert(section.to_string(), vec![content]);
            }
        } else {
            default.push(content);
        }
    }

    hm.insert("default".to_string(), default);
    hm
}

fn compile_content(dir: &str) -> io::Result<Vec<Content>> {
    let mut contents = Vec::new();
    let path = format!("{}/**/*", dir);

    for entry in glob(path.as_str()).expect("Couldn't read from {dir}") {
        if let Ok(entry) = entry {
            println!("{entry:?}");
        }
    }

    Ok(contents)
}

fn main() {
    const INPUT: &str = "content/";
    const OUTPUT: &str = "build/";

    if let Ok(content) = compile_content(INPUT) {
        let content_map = compile_content_map(&content);
        let mut context = Context::new();
        context.insert("data", &content_map);
        if let Ok(_) = create_files(content, &context) {
            println!(
                "Output files at {}",
                Path::new(OUTPUT).canonicalize().unwrap().to_string_lossy()
            );
        }
    }
}
