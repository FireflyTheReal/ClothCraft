//! Viewport presentation only. These values never affect physical contact.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntiAliasing {
    Off,
    #[default]
    Msaa4,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub anti_aliasing: AntiAliasing,
    pub gradient: bool,
    pub top: [u8; 3],
    pub bottom: [u8; 3],
    pub floor_color: [u8; 3],
    pub floor: bool,
    pub grid: bool,
    pub shadow: bool,
    pub shadow_opacity: f32,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            anti_aliasing: AntiAliasing::Msaa4,
            gradient: true,
            top: [60, 67, 78],
            bottom: [116, 126, 140],
            floor_color: [98, 107, 119],
            floor: true,
            grid: true,
            shadow: true,
            shadow_opacity: 0.3,
        }
    }
}
impl Appearance {
    pub fn sanitize(&mut self) {
        self.shadow_opacity = if self.shadow_opacity.is_finite() {
            self.shadow_opacity.clamp(0., 0.8)
        } else {
            0.3
        };
    }
    pub fn samples(&self, supports_msaa4: bool) -> u32 {
        if self.anti_aliasing == AntiAliasing::Msaa4 && supports_msaa4 {
            4
        } else {
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_preferences_default_and_msaa_fallback_are_defined() {
        let mut settings: Appearance = serde_json::from_str("{\"floor\":false}").unwrap();
        assert!(!settings.floor);
        assert_eq!(settings.samples(true), 4);
        assert_eq!(settings.samples(false), 1);
        settings.anti_aliasing = AntiAliasing::Off;
        assert_eq!(settings.samples(true), 1);
        settings.shadow_opacity = f32::NAN;
        settings.sanitize();
        assert_eq!(settings.shadow_opacity, 0.3);
    }
}
