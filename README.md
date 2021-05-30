# blog-updater

a simple interactive cli tool to generate blog websites from markdown files that are in your git repository.


## Why?

There are a lot of good static website/blog generators out there. `blog-updater` is one I made and it is not better than any of the other options out there. It has less features, and is more experimental.

So why did I make it?

- for fun
- I wanted something that works out of the box **without needing a lot of extra configuration**
- I wanted it to create blogs from my git repository, and **only update blogs that had changed since my last update**

## How?

This blog generator works by reading your git history, and finding all blog files that end in "BLOG.md". For every new blog file it finds, it parses that file and generates the markdown to HTML. It also recreates a home page with links to all of the blog files.

Additionally, you have a dedicated blogs branch which is used as a reference point to track which blog files have not been rendered yet. Every time you run this `blog-updater`, it will fast-forward the blogs branch to the main branch. This has the effect of only rendering "BLOG.md" files that have changed since the last update.

## Installation

It is written in rust, so you should be able to compile it by:

```sh
git clone https://github.com/nikita-skobov/blog-updater
cd blog-updater/blog-updater
cargo build --release
./target/release/blog-updater
```

It is interactive, so at first run, it will ask you some questions if it is confused about what you want. You can optionally make it non-interactive by doing:

```sh
./target/releast/blog-updater --no-interactive
```
