use std::{
    collections::HashMap,
    fmt::Display,
    fs,
    io::{self, BufRead, BufReader, Read, Write, BufWriter},
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
    pub frontmatter: Frontmatter,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Frontmatter(HashMap<String, String>);

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

fn read_frontmatter<R: BufRead>(reader: &mut R) -> io::Result<Frontmatter> {
    let mut hm = HashMap::new();
    let mut buf = String::new();

    while let Ok(bytes_read) = reader.read_line(&mut buf) {
        if bytes_read == 0 || buf.starts_with('-') {
            break;
        }


        if let Some((k, v)) = buf.split_once(":") {
            hm.insert(k.trim().to_string(), v.trim().to_string());
        }

        buf.clear();
    }

    Ok(Frontmatter(hm))
}

fn compile_content(dir: &str) -> io::Result<Vec<Content>> {
    let mut contents = Vec::new();
    let path = format!("{}/**/*", dir);

    for entry in glob(path.as_str()).expect("Couldn't read from {dir}") {
        if let Ok(entry) = entry {
            if entry.is_file() {
                println!("opening {entry:?}");
                let file = fs::File::open(entry.as_path())?;
                let mut reader = BufReader::new(file);
                let frontmatter = read_frontmatter(&mut reader)?;
                let buf = String::new();
                let parser = pulldown_cmark::Parser::new(buf.as_str());
                let mut content = String::new();

                if let Some((_, rest)) = path.split_once(std::path::MAIN_SEPARATOR_STR) {
                    pulldown_cmark::html::push_html(&mut content, parser);

                    contents.push(Content {
                        path: rest.to_string(),
                        frontmatter,
                        content
                    });
                }

            }
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

