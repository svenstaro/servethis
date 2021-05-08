<p align="center">
  <img src="data/logo.svg" alt="miniserve - a CLI tool to serve files and dirs over HTTP"><br>
</p>

# miniserve - a CLI tool to serve files and dirs over HTTP

[![CI](https://github.com/svenstaro/miniserve/workflows/CI/badge.svg)](https://github.com/svenstaro/miniserve/actions)
[![Docker Cloud Build Status](https://img.shields.io/docker/cloud/build/svenstaro/miniserve)](https://cloud.docker.com/repository/docker/svenstaro/miniserve/)
[![Crates.io](https://img.shields.io/crates/v/miniserve.svg)](https://crates.io/crates/miniserve)
[![license](http://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/svenstaro/miniserve/blob/master/LICENSE)
[![Stars](https://img.shields.io/github/stars/svenstaro/miniserve.svg)](https://github.com/svenstaro/miniserve/stargazers)
[![Downloads](https://img.shields.io/github/downloads/svenstaro/miniserve/total.svg)](https://github.com/svenstaro/miniserve/releases)
[![Lines of Code](https://tokei.rs/b1/github/svenstaro/miniserve)](https://github.com/svenstaro/miniserve)

**For when you really just want to serve some files over HTTP right now!**

**miniserve** is a small, self-contained cross-platform CLI tool that allows you to just grab the binary and serve some file(s) via HTTP.
Sometimes this is just a more practical and quick way than doing things properly.

## Screenshot

![Screenshot](screenshot.png)

## How to use

### Serve a directory:

    miniserve linux-distro-collection/

### Serve a single file:

    miniserve linux-distro.iso

### Require username/password:

    miniserve --auth joe:123 unreleased-linux-distros/

### Require username/password as hash:

    pw=$(echo -n "123" | sha256sum | cut -f 1 -d ' ')
    miniserve --auth joe:sha256:$pw unreleased-linux-distros/

### Generate random 6-hexdigit URL:

    miniserve -i 192.168.0.1 --random-route /tmp
    # Serving path /private/tmp at http://192.168.0.1/c789b6

### Bind to multiple interfaces:

    miniserve -i 192.168.0.1 -i 10.13.37.10 -i ::1 /tmp/myshare

### Upload a file using `curl`:

    # in one terminal
    miniserve -u .
    # in another terminal
    curl -F "path=@$FILE" http://localhost:8080/upload\?path\=/

(where `$FILE` is the path to the file. This uses miniserve's default port of 8080)

## Features

- Easy to use
- Just works: Correct MIME types handling out of the box
- Single binary drop-in with no extra dependencies required
- Authentication support with username and password (and hashed password)
- Mega fast and highly parallel (thanks to [Rust](https://www.rust-lang.org/) and [Actix](https://actix.rs/))
- Folder download (compressed on the fly as `.tar.gz` or `.zip`)
- File uploading
- Pretty themes (with light and dark theme support)
- Scan QR code for quick access
- Shell completions
- Sane and secure defaults

## Usage

    miniserve 0.14.0
    Sven-Hendrik Haase <svenstaro@gmail.com>, Boastful Squirrel <boastful.squirrel@gmail.com>
    For when you really just want to serve some files over HTTP right now!

    USAGE:
        miniserve [FLAGS] [OPTIONS] [--] [PATH]

    FLAGS:
        -D, --dirs-first
                List directories first

        -r, --enable-tar
                Enable uncompressed tar archive generation

        -g, --enable-tar-gz
                Enable gz-compressed tar archive generation

        -z, --enable-zip
                Enable zip archive generation

                WARNING: Zipping large directories can result in out-of-memory exception because zip generation is done in
                memory and cannot be sent on the fly
        -u, --upload-files
                Enable file uploading

        -h, --help
                Prints help information

        -P, --no-symlinks
                Do not follow symbolic links

        -o, --overwrite-files
                Enable overriding existing files during file upload

        -q, --qrcode
                Enable QR code display

            --random-route
                Generate a random 6-hexdigit route

        -V, --version
                Prints version information

        -v, --verbose
                Be verbose, includes emitting access logs


    OPTIONS:
        -a, --auth <auth>...
                Set authentication. Currently supported formats: username:password, username:sha256:hash,
                username:sha512:hash (e.g. joe:123,
                joe:sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3)
        -c, --color-scheme <color-scheme>
                Default color scheme [default: squirrel]  [possible values: squirrel, archlinux,
                zenburn, monokai]
        -d, --color-scheme-dark <color-scheme-dark>
                Default color scheme [default: archlinux]  [possible values: squirrel, archlinux,
                zenburn, monokai]
            --header <header>...
                Set custom header for responses
            --index <index_file>
                The name of a directory index file to serve, like "index.html"

                Normally, when miniserve serves a directory, it creates a listing for that directory. However, if a
                directory contains this file, miniserve will serve that file instead.
        -i, --interfaces <interfaces>...
                Interface to listen on

        -p, --port <port>
                Port to use [default: 8080]

            --print-completions <shell>
                Generate completion file for a shell [possible values: zsh, bash, fish,
                powershell, elvish]
        -t, --title <title>
                Shown instead of host in page title and heading


    ARGS:
        <PATH>
                Which path to serve

## How to install

<a href="https://repology.org/project/miniserve/versions"><img align="right" src="https://repology.org/badge/vertical-allrepos/miniserve.svg" alt="Packaging status"></a>

**On Linux**: Download `miniserve-linux` from [the releases page](https://github.com/svenstaro/miniserve/releases) and run

    chmod +x miniserve-linux
    ./miniserve-linux

Alternatively, if you are on **Arch Linux**, you can do

    pacman -S miniserve

**On OSX**: Download `miniserve-osx` from [the releases page](https://github.com/svenstaro/miniserve/releases) and run

    chmod +x miniserve-osx
    ./miniserve-osx

Alternatively install with [Homebrew](https://brew.sh/).

    brew install miniserve
    miniserve

**On Windows**: Download `miniserve-win.exe` from [the releases page](https://github.com/svenstaro/miniserve/releases) and run

    miniserve-win.exe

**With Cargo**: Make sure you have a recent version of Rust. Then you can run

    cargo install miniserve
    miniserve

**With Docker:** If you prefer using Docker for this, run

    docker run -v /tmp:/tmp -p 8080:8080 --rm -it svenstaro/miniserve /tmp

## Shell completions

If you'd like to make use of the built-in shell completion support, you need to run `miniserve
--print-completions <your-shell>` and put the completions in the correct place for your shell. A
few examples with common paths are provided below:

    # For bash
    miniserve --print-completions bash > ~/.local/share/bash-completion/miniserve
    # For zsh
    miniserve --print-completions zsh > /usr/local/share/zsh/site-functions/_miniserve
    # For fish
    miniserve --print-completions fish > ~/.config/fish/completions/miniserve.fish

## systemd

A hardened systemd-compatible unit file can be found in `packaging/miniserve@.service`. You could
install this to `/etc/systemd/system/miniserve@.service` and start and enable `miniserve` as a
daemon on a specific serve path `/my/serve/path` like this:

    systemctl enable --now miniserve@-my-serve-path

Keep in mind that you'll have to use `systemd-escape` to properly escape a path for this usage.

In case you want to customize the particular flags that miniserve launches with, you can use

    systemctl edit miniserve@-my-serve-path

and set the `[Service]` part in the resulting `override.conf` file. For instance:

    [Service]
    ExecStart=/usr/bin/miniserve --enable-tar --enable-zip --no-symlinks --verbose -i ::1 -p 1234 --title hello --color-scheme monokai --color-scheme-dark monokai -- %I

Make sure to leave the `%I` at the very end in place or the wrong path might be served. You
might additionally have to override `IPAddressAllow` and `IPAddressDeny` if you plan on making
miniserve directly available on a public interface.

## Binding behavior

For convenience reasons, miniserve will try to bind on all interfaces by default (if no `-i` is provided).
It will also do that if explicitly provided with `-i 0.0.0.0` or `-i ::`.
In all of the aforementioned cases, it will bind on both IPv4 and IPv6.
If provided with an explicit non-default interface, it will ONLY bind to that interface.
You can provide `-i` multiple times to bind to multiple interfaces at the same time.

## Why use this over alternatives?

- darkhttpd: Not easily available on Windows and it's not as easy as download and go.
- Python built-in webserver: Need to have Python installed, it's low performance, and also doesn't do correct MIME type handling in some cases.
- netcat: Not as convenient to use and sending directories is [somewhat involved](https://nakkaya.com/2009/04/15/using-netcat-for-file-transfers/).

## Releasing

This is mostly a note for me on how to release this thing:

- Make sure `CHANGELOG.md` is up to date.
- `cargo release --dry-run <version>`
- `cargo release <version>`
- Releases will automatically be deployed by Github Actions.
- Docker images will automatically be built by Docker Hub.
- Update Arch package.
