//! Geist and Geist Mono for the app, from the web viewer's vendored fonts
//! (viewer/vendor/fonts, SIL OFL 1.1): WOFF2 decoded, and static instances
//! at 400, 500, 600 and 700 cut from the variable fonts, because GPUI's text
//! system picks faces by weight and ignores variation axes.

use std::path::PathBuf;

use allsorts::binary::read::ReadScope;
use allsorts::tables::Fixed;
use allsorts::woff2::Woff2Font;

const WEIGHTS: [u32; 4] = [400, 500, 600, 700];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let fonts = std::env::var_os("OPTGRAPH_FONT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../viewer/vendor/fonts"));
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-env-changed=OPTGRAPH_FONT_DIR");
    for name in ["Geist", "GeistMono"] {
        let src = fonts.join(format!("{name}.woff2"));
        println!("cargo:rerun-if-changed={}", src.display());
        let data = std::fs::read(&src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
        let woff = ReadScope::new(&data).read::<Woff2Font<'_>>().expect("a WOFF2 font");
        let provider = woff.table_provider(0).expect("WOFF2 tables");
        for w in WEIGHTS {
            let (ttf, _) = allsorts::variations::instance(&provider, &[Fixed::from(w as f32)])
                .unwrap_or_else(|e| panic!("{name} at weight {w}: {e:?}"));
            std::fs::write(out.join(format!("{name}-{w}.ttf")), ttf).unwrap();
        }
    }
}
