use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
};

use glob::glob;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = match Tera::new("layouts/**/*") {
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

fn create_files(output: &str, contents: Vec<Content>, base_context: &Context) -> io::Result<()> {
    let default_layout = "index.html".to_string();
    for content in contents.iter() {
        if let Some(parent) = Path::new(&content.path).parent() {
            let path = Path::new(&output);
            let path = path.join(parent);
            let _ = fs::create_dir_all(path)?;
            if let Ok(mut context) = Context::from_serialize(content) {
                context.extend(base_context.clone());

                let layout = content
                    .frontmatter
                    .0
                    .get("layout")
                    .unwrap_or(&default_layout);

                if let Ok(result) = TEMPLATES.render(layout, &context) {
                    let file_path = Path::new(&output);
                    let mut file_path = file_path.join(&content.path);
                    file_path.set_extension("html");
                    let mut file = fs::File::create(file_path)?;
                    let _ = file.write_all(result.as_bytes());
                } else {
                    println!("Error rendering template {}: layout not found \"{}\"", content.path, layout);
                }
            }
        }
    }

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
                if let Ok(file_path) = entry.strip_prefix(dir) {
                    if let Some(file_path) = file_path.to_str() {
                        let file = fs::File::open(entry.as_path())?;
                        let mut reader = BufReader::new(file);
                        let frontmatter = read_frontmatter(&mut reader)?;
                        let mut buf = Vec::new();
                        reader.read_to_end(&mut buf)?;
                        if let Ok(str) = std::str::from_utf8(&buf) {
                            let parser = pulldown_cmark::Parser::new(str);
                            let mut content = String::new();

                            pulldown_cmark::html::push_html(&mut content, parser);

                            contents.push(Content {
                                path: file_path.to_string(),
                                frontmatter,
                                content,
                            });
                        }
                    }
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
        if let Ok(_) = create_files(OUTPUT, content, &context) {
            println!(
                "Output files at {}",
                Path::new(OUTPUT).canonicalize().unwrap().to_string_lossy()
            );
        }
    }
}
