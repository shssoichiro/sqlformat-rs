use std::hash::{DefaultHasher, Hash, Hasher};

use anstyle::RgbColor;

pub(crate) trait ToColor {
    fn to_color(&self) -> RgbColor;
}

impl<T: Hash> ToColor for T {
    fn to_color(&self) -> RgbColor {
        let mut s = DefaultHasher::new();
        self.hash(&mut s);

        let v = s.finish();

        let v = ((v >> 32) as u32) ^ (v as u32);

        let h = v as f64 / u32::MAX as f64;

        let c = okhsl::Okhsv { h, s: 0.7, v: 0.9 };
        let r = c.to_srgb();

        RgbColor(r.r, r.g, r.b)
    }
}
