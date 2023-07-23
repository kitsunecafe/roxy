# Roxy
This is a very lightweight and small static site generator.

# Usage

```rs
roxy --layouts ./layouts --input ./content --output ./build
```

## Layouts

Roxy layouts are made with [Tera](https://github.com/Keats/tera) templates. Most of the functionality of Roxy comes from Tera.

## Content

Content files are a combination of Frontmatter and Markdown (separated by dashes.)

```md
title: My first post!
---
Hello, Roxy!
```

There is one special Frontmatter field: `layout`. By default, this will be `index.html` (from whatever directory is selected as the layouts folder). Setting this field will change the template Roxy uses for this file.

```md
title: My post with a custom layout
layout: special.html
---
# Fancy!
```

Markdown is parsed using [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark).

## Output

Roxy will create the same structure that exists in the content directory.

