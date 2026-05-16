use gpui::Pixels;
use typst::layout::Point as TypstPoint;

// Helper for converting TypstPoint to GPUI Pixels, applying scale.
pub trait TypstPointExt {
    fn to_gpui_pixels(&self, scale_factor: f32) -> gpui::Point<Pixels>;
}

impl TypstPointExt for TypstPoint {
    fn to_gpui_pixels(&self, scale_factor: f32) -> gpui::Point<Pixels> {
        gpui::point(
            Pixels::from(self.x.to_pt() as f32 * scale_factor),
            Pixels::from(self.y.to_pt() as f32 * scale_factor),
        )
    }
}
