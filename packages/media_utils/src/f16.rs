/// F16u is low percison float, which encoded inside u16 for better performance and space
/// F16u is unsigned type, if you need signed type it is F16i
#[derive(Debug, PartialEq, Eq)]
pub struct F16u(u16);

impl From<F16u> for f32 {
    fn from(val: F16u) -> Self {
        val.0 as f32 / 100.0
    }
}

impl F16u {
    pub fn value(&self) -> f32 {
        self.0 as f32 / 100.0
    }
}

impl PartialOrd for F16u {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl Ord for F16u {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct F16i(i16);

impl From<F16i> for f32 {
    fn from(val: F16i) -> Self {
        val.0 as f32 / 100.0
    }
}

impl Ord for F16i {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for F16i {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}
