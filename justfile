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
