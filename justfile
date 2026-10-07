set shell := ["zsh", "-cu"]

default: test

test:
    cargo test --workspace

gallery:
    cargo run -p rogg-gallery

smoke:
    ROGG_GALLERY_SMOKE=1 cargo run -p rogg-gallery

capture page="replica" theme="dark" density="compact":
    mkdir -p target/visual/manual
    cargo run -p rogg-gallery -- --capture {{page}} --theme {{theme}} --density {{density}} --out target/visual/manual/{{page}}-{{theme}}-{{density}}.png --geometry target/visual/manual/{{page}}-{{theme}}-{{density}}.json

visual:
    cd tools/visual-diff && npm run all
    open target/visual/report.html

visual-check:
    cd tools/visual-diff && npm run check

genlog size="64M" out="target/genlog/sample.log":
    cargo run --release -p rogg-genlog -- --size {{size}} --out {{out}}

bundle kind="zip" files="10" size="64M" out="target/genlog/bundle.zip":
    mkdir -p target/genlog
    cargo run --release -p rogg-genlog -- --bundle {{kind}} --files {{files}} --size {{size}} --out {{out}}

bench:
    cargo bench -p rogg-core --bench engine
    cargo test -p rogg-core --release --test memory -- --ignored --nocapture

run *args:
    cargo run --release -p rogg-app -- {{args}}

demo:
    cargo run --release -p rogg-app -- --demo

visual-app:
    cd tools/visual-diff && ROGG_VISUAL_PAGE=app npm run all
    open target/visual-app/report.html

# Appends a log line every 100 ms to FILE (live follow by hand; Ctrl-C to stop).
live out="target/genlog/live.log":
    mkdir -p target/genlog
    touch {{out}}
    while true; do echo "$(date '+%Y-%m-%d %H:%M:%S.000')  INFO   ticker       tick $RANDOM" >> {{out}}; sleep 0.1; done

# Pipes FILE into rogg (stdin capture).
pipe file="target/genlog/sample.log":
    cat {{file}} | cargo run --release -p rogg-app
