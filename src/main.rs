use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader, Read, Seek, Write},
    path::Path,
};

use clap::{command, Parser};
use glob::glob;
use highlight_pulldown::PulldownHighlighter;
use regex::Regex;
use serde::{Deserialize, Serialize};
use syntect::{
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
};
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
    let path = format!("{dir}/**/*");
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
                    println!("Error rendering template {}: {:?}", &content.path, &err);
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

fn read_frontmatter<R: BufRead + Seek>(reader: &mut R) -> io::Result<Frontmatter> {
    let mut hm = HashMap::new();
    let mut buf = String::new();

    reader.take(3).read_to_string(&mut buf)?;
    if buf != "---".to_string() {
        // no frontmatter, reset the reader
        reader.seek(io::SeekFrom::Start(0))?;
        return Ok(Frontmatter(hm));
    }

    buf.clear();

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

fn compile_content(dir: &str, templates: &mut Tera, theme: &Theme) -> io::Result<Vec<Content>> {
    let re = Regex::new(r"/?(index)?\.?(md|html|tera)(.+)?").unwrap();
    let mut contents = Vec::new();
    let path = format!("{}/**/*", dir);
    let empty_context = Context::new();
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let highlighter = PulldownHighlighter::new(syntax_set, theme);

    for entry in glob(path.as_str()).expect(format!("Couldn't read from {dir}").as_str()) {
        if let Ok(entry) = entry {
            if entry.is_file() {
                if let Ok(file_path) = entry.strip_prefix(dir) {
                    if is_hidden(&entry) {
                        continue;
                    }

                    if let Some(ext) = file_path.extension() {
                        if let Some(ext) = ext.to_str() {
                            if !re.is_match(ext) {
                                continue;
                            }
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
                            let parser = highlighter.highlight(parser).unwrap();

                            let mut content = String::new();

                            pulldown_cmark::html::push_html(&mut content, parser.into_iter());

                            let result = templates.render_str(content.as_str(), &empty_context);
                            if let Ok(rendered) = result {
                                content = rendered;
                            } else if let Err(err) = result {
                                println!("Failed to render {file_path:?} {:?}", err);
                            }

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

fn is_hidden<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    if let Some(file_name) = path.file_name() {
        return file_name.to_string_lossy().starts_with(".");
    }

    false
}

fn copy_static(in_dir: &str, out_dir: &str) -> io::Result<()> {
    let path = format!("{in_dir}/**/*");
    let out_root = Path::new(out_dir);
    for entry in glob(path.as_str()).expect(format!("Couldn't read from {in_dir}").as_str()) {
        if let Ok(entry) = entry {
            if entry.is_file() {
                if is_hidden(&entry) {
                    continue;
                }

                if let Some(ext) = entry.extension() {
                    if !vec!["md", "html", "tera"].contains(&ext.to_str().unwrap()) {
                        if let Ok(bare_path) = entry.strip_prefix(in_dir) {
                            let out_path = out_root.clone().join(bare_path);
                            fs::copy(entry, out_path)?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
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
    #[arg(short, long, default_value = "base16-ocean.dark")]
    pub theme: String,
}

fn main() -> io::Result<()> {
    let opts = Options::parse();

    let mut templates = load_templates(&opts.layouts);

    let theme_set = ThemeSet::load_defaults();

    let theme = if let Ok(file) = fs::File::open(&opts.theme) {
        let mut reader = BufReader::new(file);
        let theme = ThemeSet::load_from_reader(&mut reader);
        theme.ok()
    } else {
        None
    };

    let default_theme = theme_set.themes.get(&opts.theme);
    let theme = theme.as_ref().or(default_theme);

    let content = compile_content(&opts.content, &mut templates, &theme.unwrap())?;

    let content_map = compile_content_map(&content);
    let mut context = Context::new();
    context.insert("data", &content_map);

    let _ = create_files(&opts.output, &templates, content, &context)?;
    let _ = copy_static(&opts.content, &opts.output);

    println!(
        "Output files at {}",
        Path::new(&opts.output)
            .canonicalize()
            .unwrap()
            .to_string_lossy()
    );

    Ok(())
}
