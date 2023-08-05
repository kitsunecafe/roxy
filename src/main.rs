use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
};

use clap::{command, Parser};
use glob::glob;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

#[derive(Serialize, Deserialize)]
struct Content {
    pub path: String,
    pub slug: String,
    pub frontmatter: Frontmatter,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Frontmatter(HashMap<String, String>);

fn load_templates(dir: &str) -> Tera {
    let path = format!("{}/**/*", dir);
    let mut tera = match Tera::new(path.as_str()) {
        Ok(t) => t,
        Err(e) => {
            println!("Parsing error(s): {}", e);
            ::std::process::exit(1);
        }
    };
    tera.autoescape_on(vec![]);
    tera
}

fn create_files(
    output: &str,
    templates: &Tera,
    contents: Vec<Content>,
    base_context: &Context,
) -> io::Result<()> {
    let default_layout = "index.html".to_string();
    for content in contents.iter() {
        if let Some(parent) = Path::new(&content.path).parent() {
            let file_stem = Path::new(&content.path).file_stem().unwrap();

            let path = Path::new(&output).join(parent);
            let path = if file_stem.is_empty() || file_stem.eq_ignore_ascii_case("index") {
                path
            } else {
                path.join(file_stem)
            };

            let _ = fs::create_dir_all(&path)?;
            if let Ok(mut context) = Context::from_serialize(content) {
                context.extend(base_context.clone());

                let layout = content
                    .frontmatter
                    .0
                    .get("layout")
                    .unwrap_or(&default_layout);

                let result = templates.render(layout, &context);
                if let Ok(result) = result {
                    let mut file_path = path.join("index");
                    file_path.set_extension("html");
                    let mut file = fs::File::create(file_path)?;
                    let _ = file.write_all(result.as_bytes());
                } else if let Err(err) = &result {
                    println!(
                        "Error rendering template {}: {}",
                        &content.path,
                        &err.to_string()
                    );

                    if let Some(source) = err.source() {
                        println!("Details: {}", source);
                    }
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

fn compile_content(dir: &str, templates: &mut Tera) -> io::Result<Vec<Content>> {
    let mut contents = Vec::new();
    let path = format!("{}/**/*", dir);
    let empty_context = Context::new();

    for entry in glob(path.as_str()).expect(format!("Couldn't read from {dir}").as_str()) {
        if let Ok(entry) = entry {
            if entry.is_file() {
                if let Ok(file_path) = entry.strip_prefix(dir) {
                    if let Some(file_name) = file_path.file_name() {
                        if file_name.to_string_lossy().starts_with(".") {
                            continue;
                        }
                    }

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

                            let result = templates.render_str(content.as_str(), &empty_context);
                            if let Ok(rendered) = result {
                                content = rendered;
                            } else if let Err(err) = result {
                                println!("Failed to render {file_path:?} {:?}", err);
                            }

                            let re = Regex::new(r"/?(index)?\.(md|html)(.+)?").unwrap();
                            let mut slug = re.replace(file_path, "").to_string();
                            slug.insert(0, '/');

                            let path = file_path.to_string();

                            contents.push(Content {
                                path,
                                slug,
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

#[derive(Parser)]
#[command(name = "Roxy")]
#[command(author = "KitsuneCafe")]
#[command(version = "1.0")]
#[command(about = "A very small static site generator", long_about = None)]
pub struct Options {
    #[arg(short, long, default_value = "build/")]
    pub output: String,
    #[arg(short, long, default_value = "content/")]
    pub content: String,
    #[arg(short, long, default_value = "layouts/")]
    pub layouts: String,
}

fn main() {
    let opts = Options::parse();

    let mut templates = load_templates(&opts.layouts);
    if let Ok(content) = compile_content(&opts.content, &mut templates) {
        let content_map = compile_content_map(&content);
        let mut context = Context::new();
        context.insert("data", &content_map);
        if let Ok(_) = create_files(&opts.output, &templates, content, &context) {
            println!(
                "Output files at {}",
                Path::new(&opts.output)
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
            );
        }
    }
}
