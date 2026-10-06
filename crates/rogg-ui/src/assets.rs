use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
struct Embedded;

/// rogg's icons first, then gpui-kit's default icons. The kit source errors on unknown paths, so it must come last.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = Embedded::get(path) {
            return Ok(Some(file.data));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = Embedded::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| SharedString::from(p.to_string()))
            .collect();
        if !path.starts_with("icons/rogg") {
            out.extend(gpui_kit::assets::Assets.list(path)?);
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}
