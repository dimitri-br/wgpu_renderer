#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, bytemuck::Zeroable)]
pub enum LightType {
    Directional = 0,
    Point = 1,
    Spot = 2,
}

impl LightType {
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => LightType::Directional,
            1 => LightType::Point,
            2 => LightType::Spot,
            _ => panic!("Invalid LightType value: {}", value),
        }
    }
}

impl From<LightType> for u32 {
    fn from(value: LightType) -> u32 {
        match value {
            LightType::Directional => 0,
            LightType::Point => 1,
            LightType::Spot => 2,
        }
    }
}

impl PartialEq<LightType> for u32 {
    fn eq(&self, other: &LightType) -> bool {
        *self == *other as u32
    }
}

unsafe impl bytemuck::Pod for LightType {}