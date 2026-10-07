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

bench:
    cargo bench -p rogg-core --bench engine
    cargo test -p rogg-core --release --test memory -- --ignored --nocapture

run *args:
    cargo run -p rogg-app -- {{args}}

demo:
    cargo run -p rogg-app -- --demo

visual-app:
    cd tools/visual-diff && ROGG_VISUAL_PAGE=app npm run all
    open target/visual-app/report.html
