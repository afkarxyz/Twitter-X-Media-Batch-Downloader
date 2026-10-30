pub(in crate::ui) const MICRO: f32 = 9.96;
pub(in crate::ui) const BODY: f32 = 14.04;
pub(in crate::ui) const SUBTITLE: f32 = 15.96;
pub(in crate::ui) const HEADING: f32 = 17.04;
pub(in crate::ui) const TITLE: f32 = 19.92;

pub(in crate::ui) fn size(value: f32) -> f32 {
    match value {
        10.0 => MICRO,
        13.0 => 12.96,
        14.0 => BODY,
        16.0 => SUBTITLE,
        17.0 => HEADING,
        20.0 => TITLE,
        _ => value,
    }
}
