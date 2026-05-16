pub trait TypstCurveExt {
    fn is_closed(&self) -> bool;
    fn is_ellipse(&self) -> bool;
}

impl TypstCurveExt for typst::visualize::Curve {
    fn is_closed(&self) -> bool {
        self.0
            .iter()
            .any(|item| matches!(item, typst::visualize::CurveItem::Close))
    }

    fn is_ellipse(&self) -> bool {
        let mut cubic_count = 0;
        let mut has_lines = false;
        for item in self.0.iter() {
            match item {
                typst::visualize::CurveItem::Cubic(_, _, _) => cubic_count += 1,
                typst::visualize::CurveItem::Line(_) => has_lines = true, // Flag if lines are present
                _ => {} // Ignore Move/Close items for cubic count
            }
        }
        // Standard Typst circles/ellipses consist of exactly 4 cubic segments.
        // If it has lines, it's a polygon or other complex curve.
        cubic_count == 4 && !has_lines
    }
}
